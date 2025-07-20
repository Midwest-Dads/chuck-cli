# Chuck 🧔

**Interactive commit selection for upstream contributions**

Chuck helps you easily select which commits from your template-based project should go back to the upstream template. No more manually figuring out what's template-worthy vs app-specific!

## The Problem Chuck Solves

When you create an app from a GitHub template and build your app, you make commits that are:

- ✅ **Template-worthy**: Shared utilities, bug fixes, improvements everyone could use
- ❌ **App-specific**: Your business logic, config, deployment scripts

Chuck makes it trivial to interactively select the good stuff and create a clean branch ready for upstream contribution.

**Works with GitHub templates via .chuckrc configuration!**

## Installation

### Prerequisites

- [GitHub CLI](https://cli.github.com/) (`brew install gh`)
- Rust (for building from source)

### Build from Source

```bash
git clone <this-repo>
cd chuck-cli
cargo build --release
cp target/release/chuck /usr/local/bin/  # or add to PATH
```

### Using the Install Script

```bash
./install.sh
```

## Usage

Chuck works with **GitHub templates** using `.chuckrc` configuration:

```bash
# 1. Create from template on GitHub (web UI)
# 2. Clone your new repo
git clone git@github.com:yourusername/your-app.git
cd your-app

# 3. Chuck reads .chuckrc and sets up template remote automatically
chuck  # Reads .chuckrc, adds remote, works!
```

## .chuckrc Configuration

Add a `.chuckrc` file to your **template repository**:

```toml
[template]
url = "git@github.com:company/web-template.git"
```

When someone creates a project from your template, this file comes with it and Chuck automatically:

1. Reads the template URL from `.chuckrc`
2. Adds it as a remote named "chuck-template"
3. Fetches the latest changes
4. Compares commits and shows the interactive selection

### Supported URL formats:

- `git@github.com:owner/repo.git` (SSH)
- `https://github.com/owner/repo.git` (HTTPS)
- `https://github.com/owner/repo` (HTTPS without .git)

## How Chuck Works

Chuck will:

1. **Read .chuckrc** to find the template repository URL
2. Add the template as a remote and fetch latest changes
3. Show you all commits since the template's latest commit
4. Let you interactively select which ones to contribute back
5. Create a clean branch with just those commits
6. Push the branch to the template repository

## Interactive Selection

Chuck shows you a terminal UI like this:

```
🧔 Chuck: 3 of 4 commits selected

┌─ Commits ─────────────────────────────────────┐┌─ Details ──────────────────────┐
│► [✓] abc1234 - Fix bug in auth middleware     ││Hash: abc1234567890abcdef...    │
│  [ ] def5678 - Add my company's payment logic ││Author: John Doe                │
│  [✓] ghi9012 - Improve database connection    ││Date: 2025-01-20 14:30         │
│  [ ] jkl3456 - Add deployment config          ││                                │
└───────────────────────────────────────────────┘│Message:                        │
                                                  │Fix bug in auth middleware      │
↑/↓/j/k: navigate │ Space: toggle │ a: all │     │                                │
n: none │ i: invert │ h/?: help │ Enter: proceed │Files:                          │
q: quit                                           │  • lib/auth.rs                 │
                                                  │  • lib/middleware.rs           │
                                                  └────────────────────────────────┘
```

**Controls:**

- `↑/↓` or `j/k` - Navigate between commits
- `Space` - Toggle selection
- `a` - Select all commits
- `n` - Select none (clear all)
- `i` - Invert selection
- `h` or `?` - Show help
- `Enter` - Create branch with selected commits
- `q` or `Esc` - Quit without doing anything

## Push and Create PR

After selecting commits, Chuck will:

1. Create a timestamped branch (e.g., `chuck/20250120-143022`)
2. Cherry-pick your selected commits onto the template's base
3. Attempt to push the branch to the template repository
4. Provide you with a URL to create the pull request

```bash
🧔 ✅ SUCCESS! All operations completed successfully.
🧔 Check the URL above to create your pull request.
```

## Requirements

- Must be run in a GitHub repository created from a template
- GitHub CLI must be installed and authenticated (`gh auth login`)
- Repository must have a `.chuckrc` file with template URL
- Repository must have commits since the template's latest commit

## Error Messages

Chuck gives helpful error messages:

- **No .chuckrc**: "No template found. Chuck needs a .chuckrc file with template URL."
- **No GitHub CLI**: "GitHub CLI not found. Install with: brew install gh"
- **Not authenticated**: "Make sure you're in a GitHub repository and authenticated with 'gh auth login'"
- **No commits**: "Looks like you haven't made any commits since the template. Get to work!"

## Example Workflow

```bash
# 1. Create from template on GitHub (web UI)
# 2. Clone your new repo
git clone git@github.com:myuser/my-awesome-app.git
cd my-awesome-app

# 3. Build your app (template already has .chuckrc)
git commit -m "Add user authentication"      # ← Template-worthy
git commit -m "Add company branding"         # ← App-specific
git commit -m "Fix database connection bug"  # ← Template-worthy
git commit -m "Deploy to our servers"       # ← App-specific

# 4. Interactive selection
chuck
# Select commits 1 and 3 using the TUI

# 5. Chuck creates branch and pushes automatically
# 6. Create PR using the provided GitHub URL
```

## Command Line Options

```bash
chuck --help     # Show help
chuck --version  # Show version
chuck --verbose  # Show detailed output during operation
```

## Version

Current version: 0.2.3
