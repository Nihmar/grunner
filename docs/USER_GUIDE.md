# Grunner User Guide

## Table of Contents
1. [Introduction](#introduction)
2. [Installation](#installation)
3. [Getting Started](#getting-started)
4. [Basic Usage](#basic-usage)
5. [Search Modes](#search-modes)
6. [Keyboard Shortcuts](#keyboard-shortcuts)
7. [Configuration](#configuration)
8. [Obsidian Integration](#obsidian-integration)
9. [Troubleshooting](#troubleshooting)
10. [FAQ](#faq)

## Introduction

Grunner is a fast, keyboard-driven application launcher for GNOME and other Linux desktops. Built with Rust and GTK4/libadwaita, it provides a modern, theme-aware interface that automatically adapts to your system's light/dark theme and accent color.

### Key Features
- **Fuzzy application search** with instant results
- **Colon command system** for file search, content grep, and more
- **Obsidian integration** for note management
- **GNOME Shell search provider** support
- **Power management** controls
- **Fully configurable** via TOML file

## Installation

### Prerequisites
- Linux desktop environment (GNOME recommended)
- GTK4 and libadwaita libraries
- Rust (for building from source)

### Installation Methods

#### Method 1: Build from Source (Recommended)
```bash
# Clone the repository
git clone https://github.com/Nihmar/grunner.git
cd grunner

# Build and install
bash build.sh
```

The `build.sh` script will:
1. Build the application in release mode
2. Install the binary to `~/.local/bin/grunner`
3. Install icons to `~/.local/share/icons/`
4. Create a `.desktop` file for GNOME Shell integration

#### Method 2: Manual Build
```bash
# Build the application
cargo build --release

# Copy to your bin directory
cp target/release/grunner ~/.local/bin/
```

### Setting Up a Keyboard Shortcut

To launch Grunner with a keyboard shortcut:

1. Open **GNOME Settings** → **Keyboard** → **View and Customize Shortcuts**
2. Click **Custom Shortcuts** → **Add Shortcut**
3. Configure as follows:
   - **Name**: `grunner`
   - **Command**: `/home/YOUR_USERNAME/.local/bin/grunner`
   - **Shortcut**: `Super+Space` (recommended) or `Alt+F2`

4. Click **Add** and test your shortcut

## Getting Started

### First Launch
When you first launch Grunner:
1. A configuration file is automatically created at `~/.config/grunner/grunner.toml`
2. The application scans your system for installed applications
3. The main window appears with a search bar ready for input

### Window Layout
```
┌─────────────────────────────────────────┐
│  [🔍] Search...                         │
├─────────────────────────────────────────┤
│  • Firefox Web Browser                  │
│    Browse the World Wide Web            │
│                                         │
│  • Files                                │
│    Access and organize files            │
│                                         │
│  • Terminal                             │
│    Use the command line                 │
│                                         │
│  ... more results ...                   │
├─────────────────────────────────────────┤
│  ⚙️  ⏻  ↻  ⏼  🚪                       │
└─────────────────────────────────────────┘
```

**Key Areas:**
1. **Search Bar**: Type to search, supports commands with `:`
2. **Results List**: Shows matching applications or command results
3. **Bottom Bar**: Settings and power management buttons

## Basic Usage

### Launching Applications
1. Press your keyboard shortcut (e.g., `Super+Space`)
2. Start typing the name of the application
3. Use arrow keys to navigate results
4. Press `Enter` to launch the selected application

**Example:**
- Type `fir` to find Firefox
- Type `ter` to find Terminal
### Quick Actions
- **Escape**: Close Grunner
- **Enter**: Launch selected item
- **Up/Down Arrow**: Navigate results
- **Page Up/Page Down**: Jump 10 items
## Search Modes

Grunner supports multiple search modes that activate automatically based on your input.

### 1. Application Search (Default)
The default mode searches all installed applications using fuzzy matching.

**Features:**
- Searches application names and descriptions
- Ranks results by relevance
- Shows icons and descriptions
- Supports terminal applications

**Example Queries:**
- `fire` → Firefox Web Browser
- `term` → GNOME Terminal, Konsole, etc.
- `image` → Image Viewer, GIMP, etc.

### 2. Colon Commands
Type `:` followed by a command name to access specialized search modes.

#### Available Commands:

The following commands are built-in fixed commands and cannot be overridden: `:f`, `:fg`, `:s`, `:ob`, `:obg`.

##### `:f` - File Search (built-in fixed command)
Searches for files in your home directory using `plocate` if available, falling back to `find` otherwise.

**Usage:** `:f search_term`

**Examples:**
- `:f invoice` → Find files containing "invoice"
- `:f .pdf` → Find PDF files
- `:f project/notes.md` → Find specific file

**Features:**
- Case-insensitive search
- Limited to home directory for privacy
- Opens files with appropriate application
- For text files: opens at specific line if available

##### `:fg` - Full-Text Grep (built-in fixed command)
Searches file contents using `ripgrep` if available, falling back to `grep` otherwise.

**Usage:** `:fg search_pattern`

**Examples:**
- `:fg TODO` → Find files containing "TODO"
- `:fg function.*name` → Find function definitions
- `:fg "error message"` → Find exact phrase

**Features:**
- Regular expression support
- Recursive search through home directory
- Shows file:line:content format
- Opens in `$EDITOR` at matching line

##### `:s` - GNOME Shell Search (built-in fixed command)
Queries GNOME Shell search providers.

**Usage:** `:s search_query`

**Supported Providers:**
- Files (document search)
- GNOME Calendar
- GNOME Contacts
- And any other installed providers

**Examples:**
- `:s meeting notes` → Search documents
- `:s john` → Search contacts

##### `:ob` - Obsidian Actions (built-in fixed command)
Provides quick access to Obsidian vault operations.

**Usage:** `:ob` or `:ob note text`

**Available Actions:**
1. **Open Vault**: Opens your Obsidian vault
2. **New Note**: Creates timestamped note in inbox
3. **Daily Note**: Opens or creates today's daily note
4. **Quick Note**: Appends text to quick note file

**Examples:**
- `:ob` → Show action buttons
- `:ob buy milk` → Create note with "buy milk"
- Select from file list to open specific note

##### `:obg` - Obsidian Vault Grep (built-in fixed command)
Searches content within your Obsidian vault.

**Usage:** `:obg search_pattern`

**Features:**
- Searches all Markdown files in vault
- Uses `ripgrep` for fast searching (falls back to `grep` if not available)
- Opens matches directly in Obsidian

### 4. Custom Commands
You can define additional colon commands beyond the built-in fixed commands (:f, :fg, :s, :ob, :obg) in the configuration file.

**Example Configuration:**
```toml
[commands]
# Search GitHub repositories
gh = "gh search repos \"$1\" --limit 10 --json fullName -q '.[].fullName' 2>/dev/null"

# Search Arch Linux AUR
aur = "yay -Ss \"$1\" 2>/dev/null | head -20"
```

**Usage:**
- `:gh neovim` → Search GitHub for Neovim repositories
- `:aur firefox` → Search AUR for Firefox packages

## Keyboard Shortcuts

### Navigation Shortcuts
| Key | Action |
|-----|--------|
| `↑` / `↓` | Move selection up/down |
| `Page Up` / `Page Down` | Jump 10 items up/down |
| `Home` / `End` | Jump to first/last item |
| `Tab` | Cycle through bottom bar buttons |

### Action Shortcuts
| Key | Action |
|-----|--------|
| `Enter` | Launch selected application/action |
| `Escape` | Close Grunner |
| `Ctrl+Q` | Quit application (when focused) |

### Search Shortcuts
| Key Combination | Action |
|----------------|--------|
| `:` | Start colon command |
| `Backspace` (empty) | Show all applications |

### Power Bar Shortcuts
When power bar is visible:
- `1` → Suspend
- `2` → Restart
- `3` → Power Off
- `4` → Log Out

## Configuration

Grunner is highly configurable through a TOML file located at:
```
~/.config/grunner/grunner.toml
```

### Opening Configuration
1. Click the ⚙️ (settings) button in Grunner's bottom bar
2. Or manually edit: `nano ~/.config/grunner/grunner.toml`

### Configuration Sections

#### Window Configuration
```toml
[window]
# Window dimensions in pixels
width = 640
height = 480
```

#### Search Configuration
```toml
[search]
# Maximum number of results to display
max_results = 64

# Delay before executing colon commands (milliseconds)
# Lower = more responsive, Higher = less flicker
command_debounce_ms = 300

# Directories to scan for .desktop files
app_dirs = [
    "/usr/share/applications",
    "/usr/local/share/applications",
    "~/.local/share/applications",
    "/var/lib/flatpak/exports/share/applications",
    "~/.local/share/flatpak/exports/share/applications",
]
```



#### Command Configuration
```toml
[commands]
# Define additional custom colon commands.
# $1 is replaced with the user's argument.
# Note: :f, :fg, :s, :ob, and :obg are built-in fixed commands and cannot be overridden.

# Custom command example: search DuckDuckGo
ddg = "curl -s 'https://api.duckduckgo.com/?q=$1&format=json&pretty=1' | jq -r '.Abstract' 2>/dev/null | head -5"
```

### Configuration Examples

#### Minimal Configuration
```toml
[window]
width = 800
height = 600

```

#### Advanced Configuration
```toml
[window]
width = 720
height = 540

[search]
max_results = 50
command_debounce_ms = 200
app_dirs = [
    "/usr/share/applications",
    "~/.local/share/applications",
]

[commands]
# Custom command examples (note: :f and :fg are built-in and cannot be overridden)
# Dictionary lookup
dict = "dict -d gcide \"$1\" 2>/dev/null | head -10"

# Web search example
web = "echo 'Searching for: $1'"

[obsidian]
vault = "~/Documents/Obsidian/Personal"
daily_notes_folder = "Journal/Daily"
new_notes_folder = "Inbox"
quick_note = "Quick Notes.md"
```

## Obsidian Integration

Grunner provides deep integration with Obsidian for quick note management.

### Setup
1. Ensure Obsidian is installed
2. Configure your vault in `grunner.toml`:
```toml
[obsidian]
vault = "~/Documents/Obsidian/MyVault"
daily_notes_folder = "Daily"
new_notes_folder = "Inbox"
quick_note = "Quick.md"
```

### Features

#### Quick Actions
Type `:ob` to access:
- **Open Vault**: Opens your main Obsidian vault
- **New Note**: Creates `YYYY-MM-DD-HH-MM-SS.md` in inbox
- **Daily Note**: Opens/creates `YYYY-MM-DD.md` in daily notes folder
- **Quick Note**: Appends text to your quick note file

#### File Search
When you type `:ob`, Grunner also shows files from your vault. Select any file to open it directly in Obsidian.

#### Content Search
Use `:obg search_term` to search through all Markdown files in your vault.

### Usage Examples

#### Quick Note Taking
```
:ob Meeting with team at 2pm
```
1. Shows Obsidian action bar
2. Select "Quick Note" to append to quick note file
3. Or select "New Note" to create dedicated note

#### Daily Journal
```
:ob Today I implemented the new feature
```
1. Select "Daily Note"
2. Text is appended to today's daily note
3. Note opens in Obsidian for further editing

#### Vault Navigation
```
:ob project
```
1. Shows files containing "project" in name
2. Select any file to open in Obsidian
3. Use arrow keys to browse vault contents

### Advanced Obsidian Usage

#### Template Support
Create a `templates/` folder in your vault and Grunner will prioritize template files when creating new notes.

#### Tag Search
Configure custom command for tag searching:
```toml
[commands]
obt = "rg -l \"#$1\" \"$OBSIDIAN_VAULT\" 2>/dev/null | head -20"
```
Usage: `:obt todo` → Find files with `#todo` tag

#### Backlink Search
```toml
[commands]
obl = "rg -l \"\\[\\[$1\\]\\]\" \"$OBSIDIAN_VAULT\" 2>/dev/null | head -20"
```
Usage: `:obl project` → Find files linking to `[[project]]`

## Troubleshooting

### Common Issues

#### Grunner Doesn't Launch
**Symptoms:** Keyboard shortcut does nothing, or error when running from terminal.

**Solutions:**
1. Check installation:
   ```bash
   ls -la ~/.local/bin/grunner
   ```
2. Verify dependencies:
   ```bash
   ldd ~/.local/bin/grunner
   ```
3. Check GNOME Shell integration:
   ```bash
   # Restart GNOME Shell (preserves session)
   killall -3 gnome-shell
   ```

#### No Applications Appear
**Symptoms:** Search returns no results, even for common applications.

**Solutions:**
1. Check application directories in config:
   ```bash
   cat ~/.config/grunner/grunner.toml | grep app_dirs
   ```
2. Verify `.desktop` files exist:
   ```bash
   ls /usr/share/applications/*.desktop | head -5
   ```
3. Rebuild application cache:
   ```bash
   rm ~/.cache/grunner/apps.bin
   # Restart Grunner
   ```

#### Colon Commands Don't Work
**Symptoms:** `:f`, `:fg`, etc., return no results.

**Solutions:**
1. Check if required tools are installed:
   ```bash
   which plocate rg  # Optional: grunner will fall back to find/grep if not installed
   ```
2. Install missing tools:
   ```bash
   # Ubuntu/Debian
   sudo apt install plocate ripgrep  # Optional for optimal performance
   
   # Fedora
   sudo dnf install plocate ripgrep  # Optional for optimal performance
   
   # Arch
   sudo pacman -S plocate ripgrep  # Optional for optimal performance
   ```
3. Update `plocate` database:
   ```bash
   sudo updatedb
   ```

#### Obsidian Integration Fails
**Symptoms:** `:ob` shows "Obsidian not configured" or actions don't work.

**Solutions:**
1. Verify configuration:
   ```bash
   cat ~/.config/grunner/grunner.toml | grep -A4 "\[obsidian\]"
   ```
2. Check vault path exists:
   ```bash
   ls ~/Documents/Obsidian/MyVault/
   ```
3. Ensure Obsidian is installed and can handle `obsidian://` URIs

#### Theme Issues
**Symptoms:** Wrong colors, doesn't follow system theme.

**Solutions:**
1. Check GTK theme:
   ```bash
   gsettings get org.gnome.desktop.interface gtk-theme
   ```
2. Reset GTK4 settings:
   ```bash
   rm -rf ~/.config/gtk-4.0
   ```
3. Restart Grunner

### Debug Mode

Enable debug logging to troubleshoot issues:

```bash
# Run with debug output
RUST_LOG=debug ~/.local/bin/grunner 2>&1 | tee grunner.log

# Check specific component
RUST_LOG=grunner::config=debug,grunner::launcher=debug ~/.local/bin/grunner
```

### Log Files

Grunner logs to:
- **Application logs**: Check terminal output when launched from command line
- **System logs**: `journalctl --user -u gnome-session` (for GNOME Shell issues)
- **Cache files**: `~/.cache/grunner/` (application cache)

## FAQ

### Q: How do I reset Grunner to default settings?
**A:** Delete the configuration file and cache:
```bash
rm ~/.config/grunner/grunner.toml