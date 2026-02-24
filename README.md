# grunner

A rofi-like application launcher for GNOME, written in Rust using GTK4 and libadwaita.  
Fast, fuzzy‑searching, with an inline calculator and system actions.

---

## Features

- 🔍 **Fuzzy search** through desktop applications (`.desktop` files)
- 🧮 **Inline calculator** – evaluate expressions while typing (e.g., `2+2`)
- ⚡ **System actions**: Suspend, Restart, Power Off, Log Out (with confirmation)
- ⚙️ **Settings button** – opens the configuration file in your default editor
- 🎨 **Adwaita‑style theming** – follows light/dark mode and the system accent colour
- ⌨️ **Keyboard navigation** (arrows, page up/down, Enter, Esc)
- 🧩 **Highly configurable** – window size, max results, app directories, calculator toggle

---

## Installation

### Dependencies

- **Rust** (edition 2024)
- **GTK4** (>= 0.10)
- **libadwaita** (>= 0.8 with `v1_6` feature)

Install them on your distribution:

**Fedora**
```bash
sudo dnf install rust gtk4-devel libadwaita-devel
```

**Ubuntu / Debian**
```bash
sudo apt install rustc cargo libgtk-4-dev libadwaita-1-dev
```

**Arch Linux**
```bash
sudo pacman -S rust gtk4 libadwaita
```

### Build from source

```bash
git clone https://github.com/yourusername/grunner.git
cd grunner
./build.sh
```

The `build.sh` script compiles the project in release mode and copies the binary to `~/.local/bin/grunner`.  
Make sure `~/.local/bin` is in your `PATH` (you can add `export PATH="$HOME/.local/bin:$PATH"` to your shell configuration).

---

## Usage

Run `grunner` from a terminal or bind it to a keyboard shortcut (e.g., in GNOME Settings → Keyboard → View and Customize Shortcuts → add custom shortcut with command `grunner`).

- **Type** to search for applications. The list updates in real time with fuzzy matching.
- **Press `Enter`** to launch the selected application.
- **Press `Esc`** to close the launcher.
- Use the **power buttons** at the bottom to suspend, restart, power off, or log out.

---

## Configuration

The configuration file is located at:  
`~/.config/grunner/grunner.toml`

If it does not exist, a default one is created when you first run `grunner`.  
You can also open it directly via the **Settings** button in the launcher.

### Example configuration

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

# Directories scanned for .desktop files.
# Use ~ for the home directory. Directories that do not exist are skipped.
app_dirs = [
    "/usr/share/applications",
    "~/.local/share/applications",
    "/var/lib/flatpak/exports/share/applications",
    "~/.local/share/flatpak/exports/share/applications",
]

[calculator]
# Enable inline calculator (evaluates expressions typed in the search bar).
enabled = true
```

---

## Keybindings

| Key               | Action                                    |
|-------------------|-------------------------------------------|
| `Esc`             | Close the launcher                        |
| `Enter` / `KP_Enter` | Launch selected app / copy calculator result |
| `↑` / `↓`         | Navigate up/down in the result list       |
| `Page Up`         | Jump 10 items up                          |
| `Page Down`       | Jump 10 items down                        |
| `Ctrl+C`          | (if nothing selected) – closes launcher   |

---

## Calculator

If the calculator is enabled (`[calculator] enabled = true`), typing a mathematical expression shows a result item at the top of the list.

- **Examples**: `2+2`, `sqrt(16)`, `3^3`, `(5+3)*2`
- Pressing `Enter` on the calculator item copies the result to the clipboard (without the `= ` prefix).

The calculator uses the [`meval`](https://crates.io/crates/meval) crate, which supports basic arithmetic, parentheses, and common functions.

---

## Power Actions

The bottom bar contains buttons for:

- **Suspend**
- **Restart**
- **Power Off**
- **Log Out**

Clicking any of them opens a confirmation dialog. Confirming executes the corresponding system command (`systemctl suspend`, `systemctl reboot`, `systemctl poweroff`, or `loginctl`/`gnome-session-quit` for logout).

---

## License

[Choose your license, e.g., MIT or GPL‑3.0]  
(Not specified in the provided source files; add your preferred license here.)

---

## Contributing

Issues and pull requests are welcome! Feel free to open a discussion for new features or improvements.