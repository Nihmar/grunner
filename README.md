# grunner

A minimal, fast, rofi-inspired application launcher for GNOME on Arch Linux, written in Rust.

Built with GTK4 and fuzzy matching — dark Catppuccin-style theme out of the box.

```
┌─────────────────────────────────────┐
│  Search applications…               │
├─────────────────────────────────────┤
│  Firefox                            │
│    Web browser                      │
│  Files                              │
│    Manage your files                │
│  Terminal                           │
│  …                                  │
└─────────────────────────────────────┘
```

---

## Dependencies

```bash
sudo pacman -S gtk4 pkg-config
```

Rust toolchain (if not already installed):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## Build & Install

```bash
# Clone / copy the project, then:
cd grunner

# Development run
cargo run

# Release build
cargo build --release

# Install to ~/.local/bin
mkdir -p ~/.local/bin
cp target/release/grunner ~/.local/bin/
```

---

## Keyboard Shortcuts

| Key            | Action                  |
|----------------|-------------------------|
| Type anything  | Fuzzy search apps       |
| `↓` / `↑`     | Navigate results        |
| `Enter`        | Launch selected app     |
| `Escape`       | Close                   |
| Click          | Launch app              |

---

## Bind a Global Shortcut in GNOME

1. Open **Settings → Keyboard → Custom Shortcuts**
2. Click **+** to add a new shortcut
3. Set:
   - **Name:** Launcher
   - **Command:** `/home/YOUR_USER/.local/bin/grunner`  
     *(or just `grunner` if `~/.local/bin` is in your PATH)*
   - **Shortcut:** `Super + Space` (or whatever you prefer)

---

## Desktop File (Optional)

If you want grunner to show up in GNOME's app grid:

```bash
cp grunner.desktop ~/.local/share/applications/
update-desktop-database ~/.local/share/applications/
```

---

## Project Structure

```
grunner/
├── Cargo.toml
├── README.md
├── grunner.desktop
└── src/
    ├── main.rs        # GTK4 app, UI, keyboard handling
    ├── launcher.rs    # .desktop file parser + fuzzy filter
    └── style.css      # Embedded stylesheet (Catppuccin Mocha)
```

---

## Customization

The entire look is controlled by `src/style.css`, which is compiled into the binary via `include_str!`. Edit it and `cargo build --release` to apply changes.

Key CSS classes:

- `.launcher-window` — the root window
- `.search-entry` — the search input
- `.app-list` — the results `ListBox`
- `.row-name` — app name label
- `.row-desc` — app description label

---

## How it Works

1. On startup, all `.desktop` files are read from:
   - `/usr/share/applications/`
   - `/usr/local/share/applications/`
   - `~/.local/share/applications/`
2. `Type=Application`, `NoDisplay=true`, and `Hidden=true` entries are filtered out.
3. As you type, entries are scored with `SkimMatcherV2` (a fast fuzzy algorithm) against app names and descriptions.
4. The top 64 results are shown, re-ranked on every keystroke.
5. Selecting an entry runs its `Exec=` field via `sh -c`, with `%f %u %F %U` field codes stripped.
