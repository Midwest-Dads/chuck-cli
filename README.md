# Chuck 🧔

**Interactive commit selection for upstream contributions**

Chuck helps you easily select which commits from your forked app should go back to the upstream template. No more manually figuring out what's template-worthy vs app-specific!

## The Problem Chuck Solves

When you create an app from a template and build your app, you make commits that are:

- ✅ **Template-worthy**: Shared utilities, bug fixes, improvements everyone could use
- ❌ **App-specific**: Your business logic, config, deployment scripts

Chuck makes it trivial to interactively select the good stuff and create a clean branch ready for upstream contribution.

**Works with both GitHub forks AND templates!**

## Installation

### Prerequisites

- [GitHub CLI](https://cli.github.com/) (`brew install gh`)
- Rust (for building from source)

### Build from Source

```bash
git clone <this-repo>
cd chuck
cargo build --release
cp target/release/chuck /usr/local/bin/  # or add to PATH
```

## Usage

Chuck works with both **GitHub forks** and **GitHub templates**:

### Option A: GitHub Forks (Automatic)

```bash
# 1. Fork template on GitHub (web UI)
# 2. Clone your fork
git clone git@github.com:yourusername/your-app.git
cd your-app

# 3. Chuck works automatically
chuck  # Auto-detects fork relationship
```

### Option B: GitHub Templates (with .chuckrc)

```bash
# 1. Create from template on GitHub (web UI)
# 2. Clone your new repo
git clone git@github.com:yourusername/your-app.git
cd your-app

# 3. Chuck reads .chuckrc and sets up template remote automatically
chuck  # Reads .chuckrc, adds remote, works!
```

## .chuckrc Configuration

For GitHub templates (or manual setup), add a `.chuckrc` file to your **template repository**:

```toml
[template]
url = "git@github.com:company/web-template.git"
```

When someone creates a project from your template, this file comes with it and Chuck automatically:

1. Reads the template URL from `.chuckrc`
2. Adds it as a remote named "template"
3. Fetches the latest changes
4. Compares commits and shows the interactive selection

### Supported URL formats:

- `git@github.com:owner/repo.git` (SSH)
- `https://github.com/owner/repo.git` (HTTPS)
- `https://github.com/owner/repo` (HTTPS without .git)

## How Chuck Works

Chuck will:

1. **Try fork detection first** (works automatically with GitHub forks)
2. **Try reading .chuckrc** (works with GitHub templates)
3. **Try existing template remote** (works if you manually added one)
4. Show you all commits since template/fork
5. Let you interactively select which ones to contribute back
6. Create a clean branch with just those commits

## Push and Create PR

```bash
git push origin chuck/20250120-143022
# Then create PR on GitHub web interface
```

## Interactive Selection

Chuck shows you a list like this:

```
🧔 Chuck: Sorting commits like a pro

Found 4 commits since you forked:

  [✓] abc1234 - Fix bug in auth middleware
      Files: lib/auth.rs, lib/middleware.rs
      "That's a keeper - everyone needs that fix"

> [ ] def5678 - Add my company's payment logic
      Files: src/payment.rs, src/config.rs
      "Nah, that stays with your app"

  [✓] ghi9012 - Improve database connection pool
      Files: lib/db.rs
      "That's good stuff right there"

  [ ] jkl3456 - Add app-specific deployment config
      Files: deploy.sh, k8s/
      "That's your problem, not theirs"

↑/↓: navigate, Space: toggle, Enter: chuck 'em back, q: quit
```

**Controls:**

- `↑/↓` - Navigate between commits
- `Space` - Toggle selection
- `Enter` - Create branch with selected commits
- `q` - Quit without doing anything

## Requirements

- Must be run in a GitHub repository (fork, template, or with manual remote setup)
- GitHub CLI must be installed and authenticated (`gh auth login`)
- Repository must have commits since the template/fork point
- For templates: `.chuckrc` file with template URL (or manual `template` remote)

## Dad Wisdom

Chuck provides helpful commentary on your commits:

**For template-worthy commits:**

- "That's a keeper - everyone needs that fix"
- "Yep, chuck that back to template"
- "That's good stuff right there"

**For app-specific commits:**

- "Nah, that stays with your app"
- "That's your problem, not theirs"
- "Keep that one to yourself, kiddo"

## Error Messages

Chuck gives friendly error messages:

- **Not a fork**: "This repository is not a fork. Chuck only works with forked repositories."
- **No GitHub CLI**: "GitHub CLI not found. Install with: brew install gh"
- **Not authenticated**: "Make sure you're in a GitHub repository and authenticated with 'gh auth login'"

## Example Workflow

```bash
# 1. Fork template on GitHub (web UI)
# 2. Clone your fork
git clone git@github.com:myuser/my-awesome-app.git
cd my-awesome-app

# 3. Build your app
git commit -m "Add user authentication"      # ← Template-worthy
git commit -m "Add company branding"         # ← App-specific
git commit -m "Fix database connection bug"  # ← Template-worthy
git commit -m "Deploy to our servers"       # ← App-specific

# 4. Interactive selection
chuck
# Select commits 1 and 3

# 5. Push and create PR
git push origin chuck/20250120-143022
# Create PR on GitHub: my-awesome-app → template
```

## License

MIT
