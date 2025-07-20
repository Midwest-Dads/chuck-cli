use anyhow::{anyhow, Result};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
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
    author: String,
    date: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChuckConfig {
    template: TemplateConfig,
}

#[derive(Debug, Deserialize, Serialize)]
struct TemplateConfig {
    url: String,
}

struct App {
    commits: Vec<Commit>,
    list_state: ListState,
    scroll_state: ScrollbarState,
    should_quit: bool,
    show_help: bool,
}

impl App {
    fn new(commits: Vec<Commit>) -> Self {
        let mut list_state = ListState::default();
        if !commits.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            scroll_state: ScrollbarState::new(commits.len()),
            commits,
            list_state,
            should_quit: false,
            show_help: false,
        }
    }

    fn next(&mut self) {
        if self.commits.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.commits.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i);
    }

    fn previous(&mut self) {
        if self.commits.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.commits.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i);
    }

    fn toggle_current(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if i < self.commits.len() {
                self.commits[i].selected = !self.commits[i].selected;
            }
        }
    }

    fn select_all(&mut self) {
        for commit in &mut self.commits {
            commit.selected = true;
        }
    }

    fn select_none(&mut self) {
        for commit in &mut self.commits {
            commit.selected = false;
        }
    }

    fn invert_selection(&mut self) {
        for commit in &mut self.commits {
            commit.selected = !commit.selected;
        }
    }

    fn get_selected(&self) -> Vec<&Commit> {
        self.commits.iter().filter(|c| c.selected).collect()
    }

    fn selected_count(&self) -> usize {
        self.commits.iter().filter(|c| c.selected).count()
    }

    fn current_commit(&self) -> Option<&Commit> {
        self.list_state.selected().and_then(|i| self.commits.get(i))
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run_app(cli)
}

fn run_app(cli: Cli) -> Result<()> {
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

    // Setup terminal for TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run interactive selection
    let mut app = App::new(commits);

    loop {
        terminal.draw(|frame| render_ui(frame, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if handle_key_event(&mut app, key) {
                break;
            }
        }
    }

    // Restore terminal properly
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Ensure terminal is fully restored and flushed
    drop(terminal);
    io::stdout().flush()?;

    // Get selected commits after TUI exits
    let selected_commits = app.get_selected();

    // Print clear separator and status
    println!("\n🧔 Exiting interactive mode...");
    println!(
        "🧔 Selected {} commits for contribution",
        selected_commits.len()
    );

    if selected_commits.is_empty() {
        println!("🧔 \"No commits selected. That's fine, take your time.\"");
        return Ok(());
    }

    // Show what commits were selected
    println!("\n🧔 Selected commits:");
    for commit in &selected_commits {
        println!("  • {} - {}", commit.short_hash, commit.message);
    }

    if cli.verbose {
        println!(
            "\n🧔 VERBOSE: About to process {} commits",
            selected_commits.len()
        );
    }

    println!("\n🧔 Creating branch and processing commits...");

    // Create branch with selected commits
    let (branch_name, timestamp) =
        create_branch_with_commits(&selected_commits, cli.verbose, &template_repo)?;

    // Get template URL for pushing
    let config = read_chuck_config()?;

    println!("\n🧔 Attempting to push to template repository...");

    // Push to template and create PR
    match push_to_template_and_create_pr(
        &branch_name,
        &config.template.url,
        &current_repo,
        &timestamp,
    ) {
        Ok(()) => {
            println!("\n🧔 ✅ SUCCESS! All operations completed successfully.");
            println!("🧔 Check the URL above to create your pull request.");
        }
        Err(e) => {
            println!("\n🧔 ⚠️  Branch created but couldn't auto-push: {}", e);
            println!("\n🧔 Manual commands to complete the process:");
            println!(
                "   git push {} {}:chuck-from-{}",
                config.template.url,
                branch_name,
                current_repo.replace("/", "-")
            );
            let template_repo_name = extract_repo_name_from_url(&config.template.url)?;
            let remote_branch_name = format!("chuck-from-{}", current_repo.replace("/", "-"));
            println!(
                "   Then create PR at: https://github.com/{}/pull/new/{}",
                template_repo_name, remote_branch_name
            );
        }
    }

    Ok(())
}

fn handle_key_event(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            if app.show_help {
                app.show_help = false;
            } else {
                app.should_quit = true;
                return true;
            }
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            return true;
        }
        KeyCode::Down | KeyCode::Char('j') => app.next(),
        KeyCode::Up | KeyCode::Char('k') => app.previous(),
        KeyCode::Char(' ') => app.toggle_current(),
        KeyCode::Char('a') => app.select_all(),
        KeyCode::Char('n') => app.select_none(),
        KeyCode::Char('i') => app.invert_selection(),
        KeyCode::Char('h') | KeyCode::Char('?') => app.show_help = !app.show_help,
        KeyCode::Enter => {
            if !app.show_help {
                return true; // Proceed with selected commits
            }
        }
        _ => {}
    }
    false
}

fn render_ui(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Create main layout
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let header_area = main_layout[0];
    let main_area = main_layout[1];
    let footer_area = main_layout[2];

    // Render header
    render_header(frame, header_area, app);

    // Render main content
    if app.show_help {
        render_help(frame, main_area);
    } else {
        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(main_area);

        let list_area = content_layout[0];
        let details_area = content_layout[1];

        render_commit_list(frame, list_area, app);
        render_commit_details(frame, details_area, app);
    }

    // Render footer
    render_footer(frame, footer_area, app);
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let selected_count = app.selected_count();
    let total_count = app.commits.len();

    let title = if selected_count > 0 {
        format!(
            "🧔 Chuck: {} of {} commits selected",
            selected_count, total_count
        )
    } else {
        format!("🧔 Chuck: {} commits found since template", total_count)
    };

    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::BOTTOM));

    frame.render_widget(header, area);
}

fn render_commit_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .commits
        .iter()
        .map(|commit| {
            let checkbox = if commit.selected { "✓" } else { " " };
            let style = if commit.selected {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let content = Line::from(vec![
                Span::styled(format!("[{}] ", checkbox), style),
                Span::styled(&commit.short_hash, Style::default().fg(Color::Yellow)),
                Span::raw(" - "),
                Span::styled(&commit.message, style),
            ]);

            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Commits")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("► ");

    frame.render_stateful_widget(list, area, &mut app.list_state);

    // Render scrollbar
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    let scrollbar_area = Rect {
        x: area.x + area.width - 1,
        y: area.y + 1,
        width: 1,
        height: area.height - 2,
    };

    frame.render_stateful_widget(scrollbar, scrollbar_area, &mut app.scroll_state);
}

fn render_commit_details(frame: &mut Frame, area: Rect, app: &App) {
    let content = if let Some(commit) = app.current_commit() {
        let mut text = vec![
            Line::from(vec![
                Span::styled("Hash: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(&commit.hash, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("Author: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&commit.author),
            ]),
            Line::from(vec![
                Span::styled("Date: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&commit.date),
            ]),
            Line::raw(""),
            Line::from(vec![Span::styled(
                "Message:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::raw(&commit.message),
            Line::raw(""),
        ];

        if !commit.files.is_empty() {
            text.push(Line::from(vec![Span::styled(
                "Files:",
                Style::default().add_modifier(Modifier::BOLD),
            )]));

            for file in &commit.files {
                text.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(file, Style::default().fg(Color::Cyan)),
                ]));
            }
        }

        Text::from(text)
    } else {
        Text::from("No commit selected")
    };

    let details = Paragraph::new(content)
        .block(
            Block::default()
                .title("Details")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(details, area);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let help_text = if app.show_help {
        "Press 'h' or '?' to close help"
    } else {
        "↑/↓/j/k: navigate │ Space: toggle │ a: all │ n: none │ i: invert │ h/?: help │ Enter: proceed │ q: quit"
    };

    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::TOP));

    frame.render_widget(footer, area);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help_text = Text::from(vec![
        Line::from(vec![Span::styled(
            "Chuck - Interactive Commit Selection",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        )]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "Navigation:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::raw("  ↑/↓ or j/k    Move cursor up/down"),
        Line::raw("  Space         Toggle commit selection"),
        Line::raw("  Enter         Proceed with selected commits"),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "Selection:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::raw("  a             Select all commits"),
        Line::raw("  n             Select none (clear all)"),
        Line::raw("  i             Invert selection"),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "Other:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::raw("  h or ?        Toggle this help"),
        Line::raw("  q or Esc      Quit application"),
        Line::raw("  Ctrl+C        Force quit"),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "About:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::raw("Chuck helps you select commits to contribute back to"),
        Line::raw("your template repository. Select the commits you want"),
        Line::raw("to share, then press Enter to create a branch and PR."),
    ]);

    let help_popup = Paragraph::new(help_text)
        .block(
            Block::default()
                .title("Help")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true });

    // Center the help popup
    let popup_area = centered_rect(80, 80, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(help_popup, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Keep all the existing business logic functions unchanged
fn read_chuck_config() -> Result<ChuckConfig> {
    let config_content =
        fs::read_to_string(".chuckrc").map_err(|_| anyhow!("No .chuckrc file found"))?;

    let config: ChuckConfig =
        toml::from_str(&config_content).map_err(|e| anyhow!("Failed to parse .chuckrc: {}", e))?;

    Ok(config)
}

fn extract_repo_name_from_url(url: &str) -> Result<String> {
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

    let template_date = get_template_latest_commit_date(template_repo)?;
    println!("🧔 Template last updated: {}", template_date);

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
                        if let Ok(commit_timestamp) = chrono::DateTime::parse_from_rfc3339(date_str)
                        {
                            if commit_timestamp > template_timestamp {
                                let short_hash = &sha[..7];
                                let files = get_commit_files(sha)?;

                                // Extract author and format date
                                let author = commit_info
                                    .get("author")
                                    .and_then(|a| a.get("name"))
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("Unknown")
                                    .to_string();

                                let date = commit_timestamp.format("%Y-%m-%d %H:%M").to_string();

                                commits.push(Commit {
                                    hash: sha.to_string(),
                                    short_hash: short_hash.to_string(),
                                    message: message.lines().next().unwrap_or(message).to_string(),
                                    files,
                                    selected: false,
                                    author,
                                    date,
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
) -> Result<(String, String)> {
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let timestamp_str = timestamp.to_string();
    let branch_name = format!("chuck/{}", timestamp_str);

    println!(
        "🧔 Creating branch with {} selected commits...",
        commits.len()
    );

    if verbose {
        println!("🧔 VERBOSE: About to create branch {}", branch_name);
    }

    let template_base_sha = get_template_base_commit(template_repo)?;

    if verbose {
        println!(
            "🧔 VERBOSE: Using template base commit: {}",
            template_base_sha
        );
    }

    let config = read_chuck_config()?;
    let template_remote_name = "chuck-template";

    if verbose {
        println!("🧔 VERBOSE: Adding template remote and fetching...");
    }

    let _ = Command::new("git")
        .args(&["remote", "add", template_remote_name, &config.template.url])
        .output();

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

    Ok((branch_name, timestamp_str))
}

fn cherry_pick_commit(commit_sha: &str, verbose: bool) -> Result<()> {
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
    timestamp: &str,
) -> Result<()> {
    let template_repo = extract_repo_name_from_url(template_url)?;
    let remote_branch_name = format!(
        "chuck-from-{}-{}",
        current_repo.replace("/", "-"),
        timestamp
    );

    println!("🧔 Executing git push command...");
    let push_command = format!(
        "git push {} {}:{}",
        template_url, branch_name, remote_branch_name
    );
    println!("🧔 Command: {}", push_command);

    let output = Command::new("git")
        .args(&[
            "push",
            template_url,
            &format!("{}:{}", branch_name, remote_branch_name),
        ])
        .output()
        .map_err(|_| anyhow!("Failed to execute git push command"))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        println!("🧔 Push failed!");
        if !stdout.is_empty() {
            println!("🧔 Git output: {}", stdout);
        }
        if !error.is_empty() {
            println!("🧔 Git error: {}", error);
        }

        return Err(anyhow!("Git push failed: {}", error));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        println!("🧔 Git output: {}", stdout);
    }

    println!("🧔 ✅ Branch pushed successfully to template repository!");

    let pr_url = format!(
        "https://github.com/{}/pull/new/{}",
        template_repo, remote_branch_name
    );

    println!("\n🧔 📝 Next step: Create your pull request");
    println!("🧔 PR URL: {}", pr_url);
    println!("🧔 Branch: {} -> {}", branch_name, remote_branch_name);
    println!("🧔 \"Now go make that pull request, kiddo!\"");

    Ok(())
}
