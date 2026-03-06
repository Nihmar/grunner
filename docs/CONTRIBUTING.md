# Contributing to grunner

First off, thanks for taking the time to contribute!

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [How to Contribute](#how-to-contribute)
- [Issue Guidelines](#issue-guidelines)
- [Pull Request Guidelines](#pull-request-guidelines)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [Commit Guidelines](#commit-guidelines)

## Code of Conduct

Please note that this project is released with a [Contributor Code of Conduct](../CODE_OF_CONDUCT.md). By participating in this project you agree to abide by its terms.

## Getting Started

### Prerequisites

To contribute to grunner, you'll need:

- **Rust** (edition 2024) - [install Rust](https://rustup.rs/)
- **GTK4** (≥ 0.10) and **libadwaita** (≥ 0.8 with `v1_6` feature)
- **Cargo** - Rust's package manager (included with Rust)

On Arch Linux:

```bash
sudo pacman -S rust gtk4 libadwaita
```

### Building from Source

```bash
git clone https://github.com/Nihmar/grunner.git
cd grunner
cargo build --release
```

The compiled binary will be at `target/release/grunner`.

## How to Contribute

### Reporting Bugs

This section guides you through submitting a bug report via GitHub.

**How to determine if it's a bug**

- First check if the issue has already been reported [search open issues](https://github.com/Nihmar/grunner/issues?q=is%3Aopen)
- Check the [FAQ](README.md) and documentation
- Ensure you're using the latest version

**How to write a good bug report**

- Use a clear and descriptive title
- Describe the steps to reproduce the bug
- Include any relevant log output (check `~/.cache/grunner/grunner.log` or use `GRUNNER_LOG=journal`)
- Include your system information (OS, GTK version, etc.)

### Suggesting Features

This section guides you through proposing new feature ideas.

**How to write a good feature request**

- Use a clear and descriptive title
- Explain why you want to add the feature and how you intend to use it
- Describe alternatives you've considered
- Be aware that feature requests are not accepted for all items

### Pull Requests

**Pre-submit checklist**

- [ ] Read the [development workflow](#development-workflow)
- [ ] Read and follow the [commit guidelines](#commit-guidelines)
- [ ] Read and follow the [pull request guidelines](#pull-request-guidelines)
- [ ] You have filled the appropriate template into the PR
- [ ] Self-review completed, all tests and lints passed

### Your First PR

To make your first pull request:

1. **Fork** the repo on GitHub
2. **Clone** the project to your machine
3. **Commit** changes to your own branch
4. **Push** your work back up to your fork
5. Submit a **Pull Request**

> **Note**: Never force push to any branches. This ensures contributors can track your work.

## Issue Guidelines

Before creating an issue, please check:

- Existing [issues](https://github.com/Nihmar/grunner/issues) for duplicates
- The [documentation](README.md) and [Error Logging](ERROR_LOGGING.md) guides

### Bug Report Template

When reporting a bug, please provide:

- **Title**: Clear, descriptive summary
- **System**: OS, desktop environment, grunner version
- **Steps to reproduce**: Numbered list of actions that trigger the issue
- **Expected behavior**: What you expected to happen
- **Actual behavior**: What actually happened
- **Logs**: Output from `GRUNNER_LOG=journal` or `~/.cache/grunner/grunner.log`
- **Screenshots**: If applicable

### Feature Request Template

When suggesting a feature, please include:

- **Use case**: Why this feature is needed
- **Description**: What the feature should do
- **Alternatives**: Other approaches considered
- **Example usage**: How users would interact with it

## Pull Request Guidelines

- **One thing at a time**: One PR should address one concern. If you want to make multiple changes, please discuss it or create multiple issues/PRs.
- **Small commits**: Keep your commits focused and meaningful.
- **Self-review**: Review your own code before submitting.
- **Tests**: Add tests for any new functionality.
- **Documentation**: Update relevant documentation.

## Development Workflow

### Project Structure

```
grunner/
├── src/
│   ├── main.rs          # Entry point
│   ├── ui.rs            # Main UI builder
│   ├── list_model.rs    # Search model
│   ├── launcher.rs      # .desktop file scanning
│   ├── app_mode.rs      # Mode detection
│   ├── item_activation.rs # Activation logic
│   ├── obsidian_bar.rs  # Obsidian action bar
│   ├── power_bar.rs     # Power actions
│   ├── settings_window.rs # Settings dialog
│   ├── utils.rs         # Utility functions
│   ├── search_provider.rs # D-Bus client
│   ├── actions.rs       # Launch, power, open file
│   ├── config.rs        # TOML loading
│   ├── logging.rs       # Logging system
│   ├── app_item.rs      # App entry wrapper
│   ├── cmd_item.rs      # Command output wrapper
│   ├── obsidian_item.rs # Obsidian entry wrapper
│   ├── search_result_item.rs # Search result wrapper
│   └── style.css        # Theme styles
├── assets/
│   ├── grunner.desktop  # Application launcher
│   └── grunner.png      # Application icon
├── docs/                 # Documentation
└── Cargo.toml
```

### Running in Development

You can run the binary directly from the build:

```bash
cargo build --release
./target/release/grunner
```

### Code Style

- Follow Rust's official style guide
- Use `rustfmt` for formatting (`cargo fmt`)
- Clippy linting is recommended (`cargo clippy`)

### Adding New Features

When adding new features:

1. **Design first**: Outline your approach
2. **Small PRs**: Keep changes focused
3. **Tests**: Include unit and integration tests
4. **Docs**: Update relevant documentation

## Testing

### Unit Tests

```bash
cargo test
```

### Integration Tests

Some tests may require a display. Run with:

```bash
export DISPLAY=:0
cargo test
```

### Logging Tests

To test logging functionality:

```bash
GRUNNER_LOG=journal cargo run
journalctl -t grunner
```

or

```bash
GRUNNER_LOG=file GRUNNER_LOG_FILE=~/test.log cargo run
cat ~/test.log
```

## Commit Guidelines

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

| Type        | Description                          |
| ----------- | ------------------------------------ |
| `feat`      | New feature                          |
| `fix`       | Bug fix                              |
| `docs`      | Documentation only changes           |
| `style`     | Formatting changes                   |
| `refactor`  | Code refactoring                     |
| `perf`      | Performance improvements             |
| `test`      | Adding tests                         |
| `chore`     | Maintenance tasks                    |

### Scope

Use descriptive scopes that indicate what part of the codebase is affected:

- `ui` - UI components
- `search` - Search functionality
- `launcher` - .desktop file handling
- `obsidian` - Obsidian integration
- `power` - Power bar actions
- `settings` - Settings dialog
- `config` - Configuration loading
- `logging` - Logging system
- `actions` - Launch/power/file actions
- `misc` - Miscellaneous

### Example Commit Messages

```
feat(ui): add settings dialog with multiple tabs

- Implement settings window with GTK4
- Add tabs for Info, General, Search, Obsidian
- Persist settings to config file
- Support graphical editing of configuration

Fix(search): correct fuzzy search ranking algorithm

- Update ranking to favor exact matches
- Improve result ordering by match position
- Reduce noise from GNOME search providers

docs(readme): update installation instructions

- Add AUR installation steps
- Clarify build dependencies
- Add troubleshooting section
```

### Commit Best Practices

- Use present tense in the subject line
- Keep the subject under 72 characters
- Capitalize the subject line
- Use an imperative mood ("add" not "added")
- Reference issues and PRs where possible

## Questions?

If you have any questions, please open an issue or discuss in the PR.

Happy contributing! 🚀
