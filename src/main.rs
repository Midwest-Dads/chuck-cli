use anyhow::{anyhow, Result};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use textwrap::{wrap, Options};
use unicode_width::UnicodeWidthStr;

#[derive(Parser)]
#[command(name = "chuck")]
#[command(about = "🧔 Chuck: Interactive commit selection for upstream contributions")]
#[command(version)]
struct Cli {
    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone)]
struct Commit {
    hash: String,
    short_hash: String,
    message: String,
    files: Vec<String>,
    selected: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChuckConfig {
    template: TemplateConfig,
}

#[derive(Debug, Deserialize, Serialize)]
struct TemplateConfig {
    url: String,
}

struct CommitSelector {
    commits: Vec<Commit>,
    current_index: usize,
}

impl CommitSelector {
    fn new(commits: Vec<Commit>) -> Self {
        Self {
            commits,
            current_index: 0,
        }
    }

    fn toggle_current(&mut self) {
        if !self.commits.is_empty() {
            self.commits[self.current_index].selected = !self.commits[self.current_index].selected;
        }
    }

    fn move_up(&mut self) {
        if self.current_index > 0 {
            self.current_index -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.current_index < self.commits.len().saturating_sub(1) {
            self.current_index += 1;
        }
    }

    fn get_selected(&self) -> Vec<&Commit> {
        self.commits.iter().filter(|c| c.selected).collect()
    }

    fn display(&self) {
        // Move cursor to top and clear to end of screen (reduces flicker)
        print!("\x1B[H\x1B[J");
        io::stdout().flush().unwrap();

        let (terminal_width, _) = size().unwrap_or((80, 24));
        let width = terminal_width as usize;

        // Header
        println!("🧔 Chuck: Sorting commits like a pro");
        println!();
        println!("Found {} commits since template:", self.commits.len());
        println!();

        // Display commits with proper wrapping
        for (i, commit) in self.commits.iter().enumerate() {
            let cursor = if i == self.current_index { ">" } else { " " };
            let checkbox = if commit.selected { "[✓]" } else { "[ ]" };

            // Main commit line with proper wrapping using textwrap
            let prefix = format!("{} {} {} - ", cursor, checkbox, commit.short_hash);
            let prefix_width = UnicodeWidthStr::width(prefix.as_str());
            let available_width = width.saturating_sub(prefix_width);

            if available_width > 0 {
                let indent_string = " ".repeat(prefix_width);
                let options = Options::new(available_width)
                    .initial_indent("")
                    .subsequent_indent(&indent_string);

                let wrapped_lines = wrap(&commit.message, &options);

                for (line_idx, line) in wrapped_lines.iter().enumerate() {
                    if line_idx == 0 {
                        println!("{}{}", prefix, line);
                    } else {
                        println!("{}", line);
                    }
                }
            } else {
                // Terminal too narrow, just print what we can
                println!("{}", prefix);
            }

            // Files line with wrapping
            if !commit.files.is_empty() {
                let file_list = if commit.files.len() <= 3 {
                    commit.files.join(", ")
                } else {
                    format!(
                        "{} and {} more",
                        commit.files[..2].join(", "),
                        commit.files.len() - 2
                    )
                };

                let files_prefix = "    Files: ";
                let files_prefix_width = UnicodeWidthStr::width(files_prefix);
                let available_width = width.saturating_sub(files_prefix_width);

                if available_width > 0 {
                    let files_indent_string = " ".repeat(files_prefix_width);
                    let options = Options::new(available_width)
                        .initial_indent("")
                        .subsequent_indent(&files_indent_string);

                    let wrapped_lines = wrap(&file_list, &options);

                    for (line_idx, line) in wrapped_lines.iter().enumerate() {
                        if line_idx == 0 {
                            println!("{}{}", files_prefix, line);
                        } else {
                            println!("{}", line);
                        }
                    }
                } else {
                    // Terminal too narrow
                    println!("{}", files_prefix);
                }
            }

            println!(); // Empty line between commits
        }

        // Footer
        println!("↑/↓: navigate, Space: toggle, Enter: chuck 'em back, q: quit");
    }
}

fn read_chuck_config() -> Result<ChuckConfig> {
    let config_content =
        fs::read_to_string(".chuckrc").map_err(|_| anyhow!("No .chuckrc file found"))?;

    let config: ChuckConfig =
        toml::from_str(&config_content).map_err(|e| anyhow!("Failed to parse .chuckrc: {}", e))?;

    Ok(config)
}

fn extract_repo_name_from_url(url: &str) -> Result<String> {
    // Extract owner/repo from various URL formats
    // git@github.com:owner/repo.git -> owner/repo
    // https://github.com/owner/repo.git -> owner/repo
    // https://github.com/owner/repo -> owner/repo

    if url.starts_with("git@github.com:") {
        let without_prefix = url.strip_prefix("git@github.com:").unwrap();
        let without_suffix = without_prefix
            .strip_suffix(".git")
            .unwrap_or(without_prefix);
        Ok(without_suffix.to_string())
    } else if url.starts_with("https://github.com/") {
        let without_prefix = url.strip_prefix("https://github.com/").unwrap();
        let without_suffix = without_prefix
            .strip_suffix(".git")
            .unwrap_or(without_prefix);
        Ok(without_suffix.to_string())
    } else {
        Err(anyhow!("Unsupported repository URL format: {}", url))
    }
}

fn find_template_repo() -> Result<String> {
    // Only try reading .chuckrc file (templates only)
    if let Ok(config) = read_chuck_config() {
        println!("🧔 Found template in .chuckrc: {}", config.template.url);
        return extract_repo_name_from_url(&config.template.url);
    }

    Err(anyhow!(
        "No template found. Chuck needs a .chuckrc file with template URL.\n  \
        Add this to your template repository:\n  \
        [template]\n  \
        url = \"git@github.com:your-org/your-template.git\""
    ))
}

fn get_current_repo() -> Result<String> {
    // Get current repository name using GitHub CLI
    let output = Command::new("gh")
        .args(&["repo", "view", "--json", "owner,name"])
        .output()
        .map_err(|_| anyhow!("GitHub CLI not found. Install with: brew install gh"))?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get current repo info. Make sure you're in a GitHub repository and authenticated with 'gh auth login'"));
    }

    let json: Value = serde_json::from_slice(&output.stdout)?;

    let owner = json
        .get("owner")
        .and_then(|o| o.get("login"))
        .and_then(|l| l.as_str())
        .ok_or_else(|| anyhow!("Could not get repository owner"))?;

    let name = json
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| anyhow!("Could not get repository name"))?;

    Ok(format!("{}/{}", owner, name))
}

fn get_template_latest_commit_date(template_repo: &str) -> Result<String> {
    // Get template's latest commit date
    let output = Command::new("gh")
        .args(&[
            "api",
            &format!("repos/{}/commits/main", template_repo),
            "--jq",
            ".commit.author.date",
        ])
        .output()
        .map_err(|_| anyhow!("Failed to get template commit info"))?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get template's latest commit date"));
    }

    let date = String::from_utf8(output.stdout)?
        .trim()
        .trim_matches('"')
        .to_string();

    Ok(date)
}

fn get_template_base_commit(template_repo: &str) -> Result<String> {
    // Get template's main branch HEAD commit SHA
    let output = Command::new("gh")
        .args(&[
            "api",
            &format!("repos/{}/commits/main", template_repo),
            "--jq",
            ".sha",
        ])
        .output()
        .map_err(|_| anyhow!("Failed to get template base commit"))?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get template's base commit SHA"));
    }

    let sha = String::from_utf8(output.stdout)?
        .trim()
        .trim_matches('"')
        .to_string();

    Ok(sha)
}

fn get_commits_since_template(current_repo: &str, template_repo: &str) -> Result<Vec<Commit>> {
    println!(
        "🧔 Comparing {} with template {}...",
        current_repo, template_repo
    );

    // Get template's latest commit date
    let template_date = get_template_latest_commit_date(template_repo)?;
    println!("🧔 Template last updated: {}", template_date);

    // Get current repo's commits
    let output = Command::new("gh")
        .args(&["api", &format!("repos/{}/commits", current_repo)])
        .output()
        .map_err(|_| anyhow!("Failed to get current repository commits"))?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get commits from current repository"));
    }

    let json: Value = serde_json::from_slice(&output.stdout)?;
    let mut commits = Vec::new();

    if let Some(commit_array) = json.as_array() {
        // Parse template date for comparison
        let template_timestamp = chrono::DateTime::parse_from_rfc3339(&template_date)?;

        for commit_data in commit_array {
            if let (Some(sha), Some(commit_info)) = (
                commit_data.get("sha").and_then(|s| s.as_str()),
                commit_data.get("commit"),
            ) {
                if let Some(message) = commit_info.get("message").and_then(|m| m.as_str()) {
                    if let Some(date_str) = commit_info
                        .get("author")
                        .and_then(|a| a.get("date"))
                        .and_then(|d| d.as_str())
                    {
                        // Only include commits newer than template
                        if let Ok(commit_timestamp) = chrono::DateTime::parse_from_rfc3339(date_str)
                        {
                            if commit_timestamp > template_timestamp {
                                let short_hash = &sha[..7];
                                let files = get_commit_files(sha)?;

                                commits.push(Commit {
                                    hash: sha.to_string(),
                                    short_hash: short_hash.to_string(),
                                    message: message.lines().next().unwrap_or(message).to_string(),
                                    files,
                                    selected: false,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(commits)
}

fn get_commit_files(sha: &str) -> Result<Vec<String>> {
    // Use git CLI instead of git2
    let output = Command::new("git")
        .args(&["show", "--name-only", "--format=", sha])
        .output()
        .map_err(|_| anyhow!("Failed to execute git show"))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to get commit files: {}", error));
    }

    let files: Vec<String> = String::from_utf8(output.stdout)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect();

    Ok(files)
}

fn create_branch_with_commits(
    commits: &[&Commit],
    verbose: bool,
    template_repo: &str,
) -> Result<String> {
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let branch_name = format!("chuck/{}", timestamp);

    println!(
        "🧔 Creating branch with {} selected commits...",
        commits.len()
    );

    if verbose {
        println!("🧔 VERBOSE: About to create branch {}", branch_name);
    }

    // Get the template's latest commit SHA to use as base
    let template_base_sha = get_template_base_commit(template_repo)?;

    if verbose {
        println!(
            "🧔 VERBOSE: Using template base commit: {}",
            template_base_sha
        );
    }

    // Add template as remote and fetch it
    let config = read_chuck_config()?;
    let template_remote_name = "chuck-template";

    if verbose {
        println!("🧔 VERBOSE: Adding template remote and fetching...");
    }

    // Add template remote (ignore error if it already exists)
    let _ = Command::new("git")
        .args(&["remote", "add", template_remote_name, &config.template.url])
        .output();

    // Fetch the template remote
    let fetch_output = Command::new("git")
        .args(&["fetch", template_remote_name])
        .output()
        .map_err(|_| anyhow!("Failed to fetch template remote"))?;

    if !fetch_output.status.success() {
        let error = String::from_utf8_lossy(&fetch_output.stderr);
        return Err(anyhow!("Failed to fetch template: {}", error));
    }

    if verbose {
        println!("🧔 VERBOSE: Template fetched successfully");
    }

    // Create branch from template base commit
    let output = Command::new("git")
        .args(&["checkout", "-b", &branch_name, &template_base_sha])
        .output()
        .map_err(|_| anyhow!("Failed to execute git checkout"))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "Failed to create branch from template base: {}",
            error
        ));
    }

    if verbose {
        println!("🧔 VERBOSE: Branch created successfully from template base");
    }

    // Cherry-pick each selected commit
    for commit in commits {
        println!(
            "🧔 Cherry-picking: {} - {}",
            commit.short_hash, commit.message
        );
        if verbose {
            println!("🧔 VERBOSE: About to cherry-pick commit {}", commit.hash);
        }

        match cherry_pick_commit(&commit.hash, verbose) {
            Ok(()) => {
                if verbose {
                    println!(
                        "🧔 VERBOSE: Cherry-pick completed for {}",
                        commit.short_hash
                    );
                }
            }
            Err(e) => {
                if e.to_string().contains("empty") {
                    println!(
                        "🧔 Skipping empty commit: {} - {}",
                        commit.short_hash, commit.message
                    );
                    // Skip empty commits
                    let skip_output = Command::new("git")
                        .args(&["cherry-pick", "--skip"])
                        .output()
                        .map_err(|_| anyhow!("Failed to skip cherry-pick"))?;

                    if !skip_output.status.success() {
                        return Err(anyhow!("Failed to skip empty cherry-pick"));
                    }
                } else {
                    return Err(e);
                }
            }
        }
    }

    println!("🧔 Created branch: {}", branch_name);
    println!("🧔 Successfully processed {} commits", commits.len());

    Ok(branch_name)
}

fn cherry_pick_commit(commit_sha: &str, verbose: bool) -> Result<()> {
    // Use git command for cherry-pick
    let output = Command::new("git")
        .args(&["cherry-pick", commit_sha])
        .output()
        .map_err(|_| anyhow!("Failed to execute git cherry-pick"))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        if verbose {
            println!("🧔 VERBOSE: Cherry-pick error: {}", error);
        }
        return Err(anyhow!("Cherry-pick failed: {}", error));
    }

    Ok(())
}

fn push_to_template_and_create_pr(
    branch_name: &str,
    template_url: &str,
    current_repo: &str,
) -> Result<()> {
    println!("🧔 Pushing branch to template repository...");

    // Extract template repo name from URL
    let template_repo = extract_repo_name_from_url(template_url)?;

    // Push branch to template repository
    let remote_branch_name = format!("chuck-from-{}", current_repo.replace("/", "-"));
    let output = Command::new("git")
        .args(&[
            "push",
            template_url,
            &format!("{}:{}", branch_name, remote_branch_name),
        ])
        .output()
        .map_err(|_| anyhow!("Failed to push to template repository"))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to push branch: {}", error));
    }

    println!("🧔 Branch pushed successfully!");

    // Generate PR URL
    let pr_url = format!(
        "https://github.com/{}/pull/new/{}",
        template_repo, remote_branch_name
    );

    println!("🧔 Create pull request at: {}", pr_url);
    println!("🧔 \"Now go make that pull request, kiddo\"");

    Ok(())
}

fn run_interactive_selection(commits: Vec<Commit>) -> Result<Vec<Commit>> {
    if commits.is_empty() {
        println!("🧔 No commits found since fork. You're all caught up!");
        return Ok(vec![]);
    }

    let mut selector = CommitSelector::new(commits);

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    loop {
        selector.display();

        match event::read()? {
            Event::Key(KeyEvent {
                code, modifiers, ..
            }) => match code {
                KeyCode::Up => selector.move_up(),
                KeyCode::Down => selector.move_down(),
                KeyCode::Char(' ') => selector.toggle_current(),
                KeyCode::Enter => break,
                KeyCode::Char('q') => {
                    disable_raw_mode()?;
                    execute!(io::stdout(), LeaveAlternateScreen)?;
                    println!("🧔 \"Alright, maybe next time\"");
                    std::process::exit(0);
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    disable_raw_mode()?;
                    execute!(io::stdout(), LeaveAlternateScreen)?;
                    println!("🧔 \"Alright, maybe next time\"");
                    std::process::exit(0);
                }
                _ => {}
            },
            Event::Resize(_, _) => {
                // Terminal was resized, re-render the display
                // The display() method will automatically get the new terminal size
            }
            _ => {}
        }
    }

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    let selected: Vec<Commit> = selector.get_selected().into_iter().cloned().collect();
    Ok(selected)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("🧔 Chuck: Let's see what you've been working on...\n");

    // Find the template repository
    let template_repo =
        find_template_repo().map_err(|e| anyhow!("🧔 \"Hmm, having trouble here\": {}", e))?;

    if cli.verbose {
        println!("🧔 VERBOSE: Template repository: {}", template_repo);
    }
    println!("🧔 Found template: {}", template_repo);

    // Get current repository
    let current_repo =
        get_current_repo().map_err(|e| anyhow!("🧔 \"Can't figure out current repo\": {}", e))?;

    if cli.verbose {
        println!("🧔 VERBOSE: Current repository: {}", current_repo);
    }

    // Get commits since template
    let commits = get_commits_since_template(&current_repo, &template_repo)
        .map_err(|e| anyhow!("🧔 \"Can't seem to get those commits\": {}", e))?;

    if commits.is_empty() {
        println!("🧔 \"Looks like you haven't made any commits since the template. Get to work!\"");
        return Ok(());
    }

    if cli.verbose {
        println!("🧔 VERBOSE: Found {} commits to review", commits.len());
        for commit in &commits {
            println!(
                "🧔 VERBOSE: {} - {} (files: {})",
                commit.short_hash,
                commit.message,
                commit.files.len()
            );
        }
    }

    // Run interactive selection
    let selected_commits = run_interactive_selection(commits)?;

    if selected_commits.is_empty() {
        println!("🧔 \"No commits selected. That's fine, take your time.\"");
        return Ok(());
    }

    if cli.verbose {
        println!(
            "🧔 VERBOSE: Selected {} commits for cherry-picking",
            selected_commits.len()
        );
    }

    // Create branch with selected commits
    let branch_name = create_branch_with_commits(
        &selected_commits.iter().collect::<Vec<_>>(),
        cli.verbose,
        &template_repo,
    )?;

    // Get template URL for pushing
    let config = read_chuck_config()?;

    // Push to template and create PR
    match push_to_template_and_create_pr(&branch_name, &config.template.url, &current_repo) {
        Ok(()) => {
            println!("🧔 All done! Check the URL above to create your pull request.");
        }
        Err(e) => {
            println!("🧔 Branch created but couldn't auto-push: {}", e);
            println!(
                "🧔 Manual push: git push {} {}:chuck-from-{}",
                config.template.url,
                branch_name,
                current_repo.replace("/", "-")
            );
        }
    }

    Ok(())
}
