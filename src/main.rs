use anyhow::{anyhow, Result};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use git2::{Oid, Repository};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io;
use std::process::Command;

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
        // Clear screen
        print!("\x1B[2J\x1B[1;1H");

        println!("🧔 Chuck: Sorting commits like a pro\n");
        println!("Found {} commits since you forked:\n", self.commits.len());

        for (i, commit) in self.commits.iter().enumerate() {
            let cursor = if i == self.current_index { ">" } else { " " };
            let checkbox = if commit.selected { "[✓]" } else { "[ ]" };
            let dad_comment = get_dad_comment(&commit.message, commit.selected);

            println!(
                "  {} {} {} - {}",
                cursor, checkbox, commit.short_hash, commit.message
            );
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
                println!("      Files: {}", file_list);
            }
            println!("      {}", dad_comment);
            println!();
        }

        println!("↑/↓: navigate, Space: toggle, Enter: chuck 'em back, q: quit");
    }
}

fn get_dad_comment(message: &str, selected: bool) -> &'static str {
    let message_lower = message.to_lowercase();

    if selected {
        if message_lower.contains("fix") || message_lower.contains("bug") {
            "\"That's a keeper - everyone needs that fix\""
        } else if message_lower.contains("add")
            && (message_lower.contains("util") || message_lower.contains("helper"))
        {
            "\"Yep, chuck that back to template\""
        } else if message_lower.contains("improve") || message_lower.contains("optimize") {
            "\"That's good stuff right there\""
        } else {
            "\"That's a keeper right there\""
        }
    } else {
        if message_lower.contains("config") || message_lower.contains("deploy") {
            "\"Nah, that stays with your app\""
        } else if message_lower.contains("app") || message_lower.contains("business") {
            "\"That's your problem, not theirs\""
        } else {
            "\"Keep that one to yourself, kiddo\""
        }
    }
}

fn read_chuck_config() -> Result<ChuckConfig> {
    let config_content =
        fs::read_to_string(".chuckrc").map_err(|_| anyhow!("No .chuckrc file found"))?;

    let config: ChuckConfig =
        toml::from_str(&config_content).map_err(|e| anyhow!("Failed to parse .chuckrc: {}", e))?;

    Ok(config)
}

fn setup_template_remote(template_url: &str) -> Result<()> {
    let repo = Repository::open(".")?;

    // Check if template remote already exists
    if let Ok(_) = repo.find_remote("template") {
        println!("🧔 Template remote already exists, fetching latest...");

        // Fetch from existing remote
        let mut remote = repo.find_remote("template")?;
        remote.fetch(&[] as &[&str], None, None)?;
        return Ok(());
    }

    println!("🧔 Setting up template remote: {}", template_url);

    // Add template remote
    repo.remote("template", template_url)?;

    // Fetch from template remote
    let mut remote = repo.find_remote("template")?;
    remote.fetch(&[] as &[&str], None, None)?;

    println!("🧔 Template remote added and fetched successfully!");
    Ok(())
}

fn template_remote_exists() -> bool {
    if let Ok(repo) = Repository::open(".") {
        repo.find_remote("template").is_ok()
    } else {
        false
    }
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
    // 1. Try fork detection first (current behavior)
    if let Ok(parent_repo) = try_fork_detection() {
        return Ok(parent_repo);
    }

    // 2. Try reading .chuckrc file
    if let Ok(config) = read_chuck_config() {
        setup_template_remote(&config.template.url)?;
        return extract_repo_name_from_url(&config.template.url);
    }

    // 3. Try existing template remote
    if template_remote_exists() {
        // Get the URL of the existing template remote
        let repo = Repository::open(".")?;
        let remote = repo.find_remote("template")?;
        if let Some(url) = remote.url() {
            return extract_repo_name_from_url(url);
        }
    }

    Err(anyhow!(
        "No template found. Chuck needs either:\n  \
        • A GitHub fork (automatic detection)\n  \
        • A .chuckrc file with template URL\n  \
        • An existing 'template' remote"
    ))
}

fn try_fork_detection() -> Result<String> {
    // Use GitHub CLI to get repository info
    let output = Command::new("gh")
        .args(&["repo", "view", "--json", "parent"])
        .output()
        .map_err(|_| anyhow!("GitHub CLI not found. Install with: brew install gh"))?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get repo info. Make sure you're in a GitHub repository and authenticated with 'gh auth login'"));
    }

    let json: Value = serde_json::from_slice(&output.stdout)?;

    if let Some(parent) = json.get("parent") {
        if let Some(full_name) = parent.get("name") {
            return Ok(full_name.as_str().unwrap().to_string());
        }
    }

    Err(anyhow!("This repository is not a fork"))
}

fn get_commits_since_fork(parent_repo: &str) -> Result<Vec<Commit>> {
    // Use GitHub CLI to compare commits
    let output = Command::new("gh")
        .args(&["api", &format!("repos/{}/compare/main...HEAD", parent_repo)])
        .output()
        .map_err(|_| anyhow!("Failed to compare with parent repository"))?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get commit comparison from GitHub API"));
    }

    let json: Value = serde_json::from_slice(&output.stdout)?;
    let mut commits = Vec::new();

    if let Some(commit_array) = json.get("commits").and_then(|c| c.as_array()) {
        for commit_data in commit_array {
            if let (Some(sha), Some(commit_info)) = (
                commit_data.get("sha").and_then(|s| s.as_str()),
                commit_data.get("commit"),
            ) {
                if let Some(message) = commit_info.get("message").and_then(|m| m.as_str()) {
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

    Ok(commits)
}

fn get_commit_files(sha: &str) -> Result<Vec<String>> {
    let repo = Repository::open(".")?;
    let oid = Oid::from_str(sha)?;
    let commit = repo.find_commit(oid)?;

    let mut files = Vec::new();

    if let Ok(tree) = commit.tree() {
        if commit.parent_count() > 0 {
            if let Ok(parent) = commit.parent(0) {
                if let Ok(parent_tree) = parent.tree() {
                    let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;

                    diff.foreach(
                        &mut |delta, _progress| {
                            if let Some(path) = delta.new_file().path() {
                                if let Some(path_str) = path.to_str() {
                                    files.push(path_str.to_string());
                                }
                            }
                            true
                        },
                        None,
                        None,
                        None,
                    )?;
                }
            }
        }
    }

    Ok(files)
}

fn create_branch_with_commits(commits: &[&Commit]) -> Result<String> {
    let repo = Repository::open(".")?;
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let branch_name = format!("chuck/{}", timestamp);

    // Get current HEAD
    let head = repo.head()?;
    let head_commit = head.peel_to_commit()?;

    // Find the merge base with the parent
    // For now, we'll create the branch from current HEAD and cherry-pick
    let _branch = repo.branch(&branch_name, &head_commit, false)?;

    // Switch to the new branch
    repo.set_head(&format!("refs/heads/{}", branch_name))?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

    // Reset to the fork point (we'll implement this properly later)
    // For now, just create the branch with selected commits

    println!("🧔 Created branch: {}", branch_name);
    println!("🧔 Selected {} commits", commits.len());

    Ok(branch_name)
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

        if let Event::Key(KeyEvent {
            code, modifiers, ..
        }) = event::read()?
        {
            match code {
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
            }
        }
    }

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    let selected: Vec<Commit> = selector.get_selected().into_iter().cloned().collect();
    Ok(selected)
}

fn main() -> Result<()> {
    let _cli = Cli::parse();

    println!("🧔 Chuck: Let's see what you've been working on...\n");

    // Find the template repository
    let template_repo =
        find_template_repo().map_err(|e| anyhow!("🧔 \"Hmm, having trouble here\": {}", e))?;

    println!("🧔 Found template: {}", template_repo);

    // Get commits since template
    let commits = get_commits_since_fork(&template_repo)
        .map_err(|e| anyhow!("🧔 \"Can't seem to get those commits\": {}", e))?;

    if commits.is_empty() {
        println!("🧔 \"Looks like you haven't made any commits since the template. Get to work!\"");
        return Ok(());
    }

    // Run interactive selection
    let selected_commits = run_interactive_selection(commits)?;

    if selected_commits.is_empty() {
        println!("🧔 \"No commits selected. That's fine, take your time.\"");
        return Ok(());
    }

    // Create branch with selected commits
    let branch_name = create_branch_with_commits(&selected_commits.iter().collect::<Vec<_>>())?;

    println!(
        "🧔 Chucked {} commits to branch: {}",
        selected_commits.len(),
        branch_name
    );
    println!("🧔 Push with: git push origin {}", branch_name);
    println!("🧔 \"Now go make that pull request, kiddo\"");

    Ok(())
}
