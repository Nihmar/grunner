# Agent Guidelines for grunner

This document provides guidelines for AI agents working on the grunner codebase.

## Build Commands

```bash
cargo build                  # debug build
cargo build --release        # optimised release build
cargo run                    # run debug
cargo run --release          # run release
```

Binary: `target/debug/grunner` or `target/release/grunner`.

## Linting & Formatting

```bash
cargo fmt                                    # format code
cargo clippy                                 # standard lints
cargo clippy -- -W clippy::pedantic          # strict lints
```

- Always run `cargo fmt` before committing.
- The project targets zero clippy warnings.

## Testing

```bash
cargo test                           # all tests (unit + integration + doctests)
cargo test -- --nocapture            # show println! output
cargo test test_function_name        # single test by partial name
cargo test module_name::             # all tests in a module
cargo test --tests                   # integration tests only
cargo test -p grunner --lib          # library unit tests only
```

> **Note:** Tests needing a display: `export DISPLAY=:0` (or appropriate display).

## Code Style

### Formatting
- `cargo fmt` enforces standard Rust style (4-space indent, 100-char lines).

### Naming Conventions
- Variables / functions: `snake_case`
- Types / structs: `PascalCase`
- Constants / statics: `SCREAMING_SNAKE_CASE`
- Enum variants: `PascalCase`
- Modules: `snake_case`

### Imports
Organize in this order, separated by a blank line:
1. Standard library (`std`, `core`, `alloc`)
2. External crates (`gtk4`, `libadwaita`, `serde`, etc.)
3. Internal modules (`crate::...`)

```rust
use std::path::PathBuf;

use gtk4::prelude::*;
use libadwaita::Application;
use log::{debug, error, info};

use crate::core::config::Config;
use crate::utils::expand_home;
```

For GTK/libadwaita, always import `gtk4::prelude::*` and `libadwaita::prelude::*`.

### Error Handling
- Use `Result<T, E>` for fallible operations.
- Prefer `match` on `Result`/`Option` over `unwrap()`/`expect()` in production code.
- Log errors with `log` macros (`error!`, `warn!`); show user-facing errors via the UI.
- When a function can't proceed, log the error and return a safe default (see `config::load()`).

### Logging
The project uses `log` + `simplelog`. Use `debug!`, `info!`, `warn!`, `error!`, `trace!`.

```bash
GRUNNER_LOG=journal cargo run     # systemd journal
GRUNNER_LOG=file cargo run        # ~/.cache/grunner/grunner.log (default)
GRUNNER_LOG=stderr cargo run      # stderr
GRUNNER_LOG=off cargo run         # disabled
```

### Async Programming
- Uses `tokio` and `futures`; mark async functions with `async fn`, use `.await` directly.
- For D-Bus, see `providers/dbus/query.rs` and `actions/workspace.rs`.

### Documentation
- Public APIs: `///` Rustdoc with args, returns, errors.
- Module-level: `//!`.
- Keep comments concise and current.

### Pattern Matching
- Prefer `match` for multiple variants; `if let` for single-case extraction.
- Use `unwrap_or`, `unwrap_or_else`, `map`, `and_then` where appropriate.

### GObject/GTK4 Patterns
- Use `glib::clone!` macro for closure captures in signals.
- Use `imp()` pattern for GObject properties: `self.imp().property.borrow()`.
- New GObject types: `#[glib::object_subclass]` + `ObjectImpl` + `glib::wrapper!` (see `model/items/` and `core/callbacks.rs`).
- Custom signals: define in `ObjectImpl::signals()` via `Signal::builder("name").build()`.
- Emit with `self.emit_by_name::<()>("signal-name", &[])`.
- Prefer builder pattern: `Widget::builder().property(value).build()`.
- Always import `prelude::*` for GTK trait methods.

### Constants & Configuration
- `const` for compile-time constants; `static` for lazy-init (`OnceLock`).
- Config lives at `~/.config/grunner/grunner.toml`; see `core::config::load()`.

## Commit Message Format

Conventional commits: `<type>(<scope>): <subject>`

| Type       | Scope examples                         |
|------------|----------------------------------------|
| `feat`     | `ui`, `search`, `launcher`, `obsidian` |
| `fix`      | `power`, `settings`, `config`          |
| `docs`     | `misc`                                 |
| `refactor` | —                                      |
| `perf`     | —                                      |
| `test`     | —                                      |
| `chore`    | —                                      |

- Present tense, imperative mood ("add" not "added")
- Subject under 72 chars, capitalize first letter

## Additional Notes

- GTK4/libadwaita GUI app — respect GNOME HIG for UI changes.
- Config is TOML; changes to `config.rs` must maintain backward compatibility.
- No Cursor or Copilot-specific rules in the repository.
- **Runtime dependency:** workspace bar requires [window-calls](https://extensions.gnome.org/extension/4724/window-calls/) GNOME Shell extension for D-Bus window info on Wayland.
