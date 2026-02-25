# grunner

A rofi‑like application launcher for GNOME, written in Rust using GTK4 and libadwaita.  
Fast, fuzzy‑searching, with an inline calculator, custom colon commands, and integrated Obsidian vault actions.

---

## DISCLAIMER

This project is vibecoded using Deepseek and Claude (free version). That's all folks.

---

## Features

- 🔍 **Fuzzy search** through desktop applications (`.desktop` files)
- 🧮 **Inline calculator** – evaluate expressions while typing (e.g., `2+2`, `16^(1/4)`)
- ⚡ **System actions**: Suspend, Restart, Power Off, Log Out (with confirmation)
- ⚙️ **Settings button** – opens the configuration file in your default editor
- ⌨️ **Keyboard navigation** (arrows, page up/down, Enter, Esc)
- 🎨 **Adwaita‑style theming** – follows light/dark mode and the system accent colour
- 🧩 **Highly configurable** – window size, max results, app directories, calculator toggle, command debounce delay
- **Colon commands** – define your own shell‑based commands (e.g., `:f pattern` to find files)
- **Obsidian integration** – open vaults, create daily/quick notes, search filenames and content
- **Smart file opening** – for command results like `file:line:content`, opens at the correct line in `$EDITOR`

---

## Installation

### Dependencies

- **Rust** (edition 2024)
- **GTK4** (>= 0.10)
- **libadwaita** (>= 0.8 with `v1_6` feature)

Install them on your distribution:

**Fedora**
```bash
sudo dnf install rust gtk4-devel libadwaita-devel plocate
```

**Ubuntu / Debian**
```bash
sudo apt install rustc cargo libgtk-4-dev libadwaita-1-dev plocate
```

**Arch Linux**
```bash
sudo pacman -S rust gtk4 libadwaita plocate
```

After installation, enable the auto-update of `plocate`'s index:

```bash
sudo updatedb
sudo systemctl enable --now plocate-updatedb.timer
```


### Build from source

```bash
rm -rf grunner
git clone https://github.com/Nihmar/grunner.git
cd grunner
./build.sh
```

The `build.sh` script compiles the project in release mode and copies the binary to `~/.local/bin/grunner`.  
Make sure `~/.local/bin` is in your `PATH` (add `export PATH="$HOME/.local/bin:$PATH"` to your shell configuration).

---

## Usage

Run `grunner` from a terminal or bind it to a keyboard shortcut (e.g., in GNOME Settings → Keyboard → View and Customize Shortcuts → add custom shortcut with command `grunner`).

- **Type** to search for applications. The list updates in real time with fuzzy matching.
- **Press `Enter`** to launch the selected application, copy a calculator result, or execute a colon command result.
- **Press `Esc`** to close the launcher.
- Use the **power buttons** at the bottom to suspend, restart, power off, or log out.

---

## Configuration

The configuration file is located at:  
`~/.config/grunner/grunner.toml`

If it does not exist, a default one is created when you first run `grunner`.  
You can also open it directly via the **Settings** button in the launcher.

### Example configuration

Below is the default configuration with all available options.  
Uncomment and adjust values as needed.

```toml
# grunner configuration
# All values are optional – missing keys fall back to built‑in defaults.

[window]
# Width and height of the launcher window in pixels.
width  = 640
height = 480

[search]
# Maximum number of fuzzy‑search results shown (only when a query is active).
max_results = 64

# Delay in milliseconds before executing a colon command (e.g. :f, :ob) after you stop typing.
# Lower values feel more responsive but may cause flickering if your command is very fast.
command_debounce_ms = 300

# Directories scanned for .desktop files.
# Use ~ for the home directory. Directories that do not exist are skipped.
app_dirs = [
    "/usr/share/applications",
    "/usr/local/share/applications",
    "~/.local/share/applications",
    "/var/lib/flatpak/exports/share/applications",
    "~/.local/share/flatpak/exports/share/applications",
]

[calculator]
# Enable inline calculator (evaluates expressions typed in the search bar).
enabled = false

[commands]
# Define colon commands. The key is the command name (without the leading ':').
# The value is a shell command that will be executed with 'sh -c'.
# Use "$1" for the argument typed after the command.
f  = "find ~ -iname \"*$1*\" 2>/dev/null | head -20"
fg = "rg --with-filename --line-number --no-heading \"$1\" ~ 2>/dev/null | head -20"

# [obsidian]
# Uncomment and fill in to enable Obsidian integration.
# vault = "~/Documents/Obsidian/MyVault"
# daily_notes_folder = "Daily"
# new_notes_folder = "Inbox"
# quick_note = "Quick.md"
```

---

## Colon Commands

When you type a colon (`:`) followed by a command name and an optional argument, `grunner` executes the associated shell command and displays its output lines as selectable items.

- **`:f pattern`** – find files (default command, uses `find`)
- **`:fg pattern`** – grep inside files (default command, uses `ripgrep`)
- **`:ob ...`** – Obsidian actions (see below)
- You can add your own commands in the `[commands]` section of the config.

Selecting a result line that looks like `file:line:content` will open the file at that line in your `$EDITOR`. If the line is a plain file path, it opens with `xdg-open`. Otherwise, the line is copied to the clipboard.

---

## Obsidian Integration

If you configure the `[obsidian]` section, two special colon commands become available:

- **`:ob`** – shows action buttons (Open Vault, New Note, Daily Note, Quick Note) above the power bar.  
  If you type `:ob something`, it searches for filenames inside your vault using `find`.
- **`:obg pattern`** – searches the *content* of all notes in your vault using `ripgrep` and displays matching lines.

### Obsidian Actions

When you click one of the buttons (or select the corresponding item after a search):

| Action       | Behaviour                                                                                                                                                     |
|--------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Open Vault   | Opens your vault in Obsidian using the `obsidian://open` URI.                                                                                                 |
| New Note     | Creates a new file in the folder specified by `new_notes_folder`. The filename includes a timestamp. If you typed text after `:ob`, that text becomes the note’s content. |
| Daily Note   | Opens (or creates) today’s daily note in the `daily_notes_folder`. If you typed text, it is appended to the note.                                            |
| Quick Note   | Opens (or creates) the file specified by `quick_note`. If you typed text, it is appended.                                                                    |

All actions fall back to the `xdg-open` URI scheme, so Obsidian must be installed and able to handle `obsidian://` links.

---

## Calculator

If the calculator is enabled (`[calculator] enabled = false`), typing a mathematical expression shows a result item at the top of the list.

- **Examples**: `2+2`, `16^(1/4)`, `3^3`, `(5+3)*2`
- **NOT WORKING**: Pressing `Enter` on the calculator item copies the result to the clipboard (without the `= ` prefix).

The calculator uses the [`evalexpr`](https://crates.io/crates/evalexpr) crate, which supports basic arithmetic, parentheses, and common functions. It automatically converts integers to floats so that division yields decimal results (e.g., `7/5` → `1.4`). If the full expression cannot be evaluated, it tries the longest valid prefix.

---

## Power Actions

The bottom bar contains buttons for:

- **Suspend**
- **Restart**
- **Power Off**
- **Log Out**

Clicking any of them opens a confirmation dialog. Confirming executes the corresponding system command (`systemctl suspend`, `systemctl reboot`, `systemctl poweroff`). Logout attempts several methods: `loginctl terminate-session`, `gnome-session-quit`, or `loginctl terminate-user`.

---

## Keybindings

| Key               | Action                                    |
|-------------------|-------------------------------------------|
| `Esc`             | Close the launcher                        |
| `Enter` / `KP_Enter` | Launch selected app / copy calculator result / activate command result |
| `↑` / `↓`         | Navigate up/down in the result list       |
| `Page Up`         | Jump 10 items up                          |
| `Page Down`       | Jump 10 items down                        |
| `Ctrl+C`          | (if nothing selected) – closes launcher   |

---

## License

MIT License

Copyright (c) 2026 Nihmar

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

---

## Contributing

Issues and pull requests are welcome! Feel free to open a discussion for new features or improvements.
