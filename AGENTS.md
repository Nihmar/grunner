# Agent Guidelines for grunner

This document provides guidelines for AI agents working on the grunner codebase. It covers build commands, linting, testing, code style, and other conventions.

## Build Commands

```bash
# Build debug version
cargo build

# Build release version (optimized)
cargo build --release

# Run the application (debug)
cargo run

# Run the application (release)
cargo run --release
```

The compiled binary will be at `target/debug/grunner` or `target/release/grunner`.

## Linting & Formatting

```bash
# Format code with rustfmt
cargo fmt

# Run clippy lints
cargo clippy

# Run clippy with stricter checks
cargo clippy -- -W clippy::pedantic
```

- Always run `cargo fmt` before committing to ensure consistent formatting.
- Address all clippy warnings; the project aims for zero warnings.

## Testing

```bash
# Run all tests
cargo test

# Run tests with output (show println! output)
cargo test -- --nocapture

# Run a specific test
cargo test test_function_name

# Run tests in a specific module
cargo test module_name::

# Run integration tests (none currently)
cargo test --tests
```

**Note:** There are currently no unit tests in the codebase, but new features should include tests. Tests that require a display can be run with `export DISPLAY=:0` (or appropriate display).

## Code Style Guidelines

### Formatting
- Use `cargo fmt` to enforce standard Rust style.
- Maximum line length: 100 characters (rustfmt default).
- Use spaces (4 spaces per indent).

### Naming Conventions
- **Variables and functions:** `snake_case`
- **Types and structs:** `PascalCase`
- **Constants and static variables:** `SCREAMING_SNAKE_CASE`
- **Enum variants:** `PascalCase`
- **Module names:** `snake_case`

### Imports Ordering
Organize imports in the following order, separated by a blank line:
1. Standard library (`std`, `core`, `alloc`)
2. External crates (`gtk4`, `libadwaita`, `serde`, etc.)
3. Internal modules (`crate::...`)

Example:
```rust
use std::path::PathBuf;
use std::env;

use gtk4::prelude::*;
use libadwaita::Application;
use log::{debug, error, info};

use crate::utils::expand_home;
use crate::config::Config;
```

### Error Handling
- Use `Result<T, E>` for fallible operations.
- Prefer `match` on `Result`/`Option` over `unwrap()`/`expect()` in production code.
- Log errors using the `log` macros (`error!`, `warn!`, `debug!`, `info!`).
- For user‑facing errors, display a descriptive message via the UI.
- When a function can’t proceed, log the error and return a default/safe value (see `config::load()` for an example).

### Logging
The project uses the `log` crate with `simplelog` backend. Use the following macros:

```rust
use log::{debug, error, info, trace, warn};

debug!("Loading configuration from {:?}", path);
info!("Application started");
warn!("Fallback to default value");
error!("Failed to read file: {}", e);
```

Logging can be configured via environment variables:
```bash
GRUNNER_LOG=journal cargo run      # log to systemd journal
GRUNNER_LOG=file cargo run         # log to file (default: ~/.cache/grunner/grunner.log)
GRUNNER_LOG=stderr cargo run       # log to stderr
GRUNNER_LOG=off cargo run          # disable logging
```

### Async Programming
- The project uses `tokio` and `futures` for asynchronous operations.
- Mark async functions with `async fn`.
- Use `.await` directly; avoid manual `poll` unless necessary.
- For D‑Bus communication, see `search_provider.rs` and `workspace_bar.rs` for examples.

### Documentation
- Document public APIs with Rustdoc (`///` comments).
- Include examples, arguments, return values, and error conditions.
- Use `//!` for module‑level documentation.
- Keep comments concise and up‑to‑date.

### Pattern Matching
- Prefer `match` over `if let` when handling multiple variants.
- Use `if let` for single‑case extraction.
- Use `unwrap_or`, `unwrap_or_else`, `map`, `and_then` where appropriate.

### Constants & Configuration
- Define constants in the appropriate module (e.g., `config.rs`).
- Use `const` for compile‑time constants, `static` for mutable global state (rare).
- Configuration is loaded from `~/.config/grunner/grunner.toml`; see `config::load()`.

## Commit Message Format

Follow the conventional commit style: `<type>(<scope>): <subject>` with optional body and footer.

### Types & Scopes

| Type      | Description                          | Common Scopes                          |
|-----------|--------------------------------------|----------------------------------------|
| `feat`    | New feature                          | `ui`, `search`, `launcher`, `obsidian` |
| `fix`     | Bug fix                              | `power`, `settings`, `config`, `logging` |
| `docs`    | Documentation only changes           | `misc`                                 |
| `style`   | Formatting changes                   |                                        |
| `refactor`| Code refactoring                     |                                        |
| `perf`    | Performance improvements             |                                        |
| `test`    | Adding tests                         |                                        |
| `chore`   | Maintenance tasks                    |                                        |

**Commit Best Practices:**
- Use present tense, imperative mood ("add" not "added")
- Keep subject under 72 characters, capitalize first letter
- Reference issues and PRs where possible

## Additional Notes

- The application is a GTK4/libadwaita GUI program; UI changes must respect GNOME HIG.
- Configuration is stored as TOML; changes to `config.rs` should maintain backward compatibility.
- Logging is essential for debugging; include appropriate `debug!`/`info!` statements in new code.
- No Cursor or Copilot‑specific rules are present in the repository.

## Runtime Dependencies

### GNOME Shell Extensions

- **window-calls** (required for workspace bar feature): https://extensions.gnome.org/extension/4724/window-calls/

  This extension provides D-Bus access to window information on Wayland. Without it, the workspace bar will not display open windows.

