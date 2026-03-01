# Grunner Development Guide

## Table of Contents
1. [Development Environment Setup](#development-environment-setup)
2. [Build System](#build-system)
3. [Code Organization](#code-organization)
4. [Testing](#testing)
5. [Debugging](#debugging)
6. [Performance Profiling](#performance-profiling)
7. [Code Style Guidelines](#code-style-guidelines)
8. [Contributing](#contributing)
9. [Release Process](#release-process)

## Development Environment Setup

### Prerequisites

#### Required Tools
- **Rust**: Version 1.70 or higher
- **Cargo**: Rust package manager (included with Rust)
- **GTK4 Development Libraries**: Version 4.6 or higher
- **libadwaita Development Libraries**: Version 1.6 or higher

#### Installation by Distribution

**Fedora/RHEL/CentOS:**
```bash
sudo dnf install rust cargo gtk4-devel libadwaita-devel
```

**Ubuntu/Debian:**
```bash
sudo apt install rustc cargo libgtk-4-dev libadwaita-1-dev
```

**Arch Linux:**
```bash
sudo pacman -S rust gtk4 libadwaita
```

**macOS (with Homebrew):**
```bash
brew install rust gtk4 libadwaita
```

### Project Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/Nihmar/grunner.git
   cd grunner
   ```

2. **Install Rust dependencies:**
   ```bash
   cargo fetch
   ```

3. **Verify build:**
   ```bash
   cargo build
   ```

4. **Run the application:**
   ```bash
   cargo run
   ```

### IDE Configuration

#### Visual Studio Code
Recommended extensions:
- **rust-analyzer**: Rust language support
- **CodeLLDB**: Debugging support
- **Even Better TOML**: TOML configuration support
- **GitLens**: Git integration

Configuration (`.vscode/settings.json`):
```json
{
    "rust-analyzer.check.command": "clippy",
    "rust-analyzer.cargo.features": "all",
    "editor.formatOnSave": true,
    "editor.rulers": [100],
    "files.trimTrailingWhitespace": true
}
```

#### IntelliJ IDEA / CLion
- Install Rust plugin
- Configure Rust toolchain
- Enable Clippy inspections

### Development Dependencies

Additional tools for development:
```bash
# Code formatting
cargo install rustfmt

# Linting
cargo install clippy

# Documentation
cargo install cargo-doc

# Testing utilities
cargo install cargo-nextest

# Performance profiling
cargo install flamegraph
```

## Build System

### Cargo Configuration

The project uses standard Cargo with the following configuration:

**Cargo.toml key sections:**
```toml
[package]
name = "grunner"
version = "0.7.0"
edition = "2024"
description = "A rofi-like application launcher for GNOME, written in Rust"

[dependencies]
# Core GUI libraries
gtk4 = "0.10"
libadwaita = { version = "0.8", features = ["v1_6"] }
glib = "0.21"

# Search and matching
fuzzy-matcher = "0.3"
regex = "1.10"

# Configuration
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"

# System integration
zbus = "5.14.0"
tokio = { version = "1.49.0", features = ["rt", "rt-multi-thread", "time", "macros", "net"] }

# Utilities
chrono = "0.4"
urlencoding = "2.1"
rayon = "1"
bincode = "1"

[profile.release]
lto = true
codegen-units = 1
opt-level = 3
panic = "abort"
strip = true
incremental = false
overflow-checks = false
```

### Build Commands

**Development build:**
```bash
cargo build
# Binary: target/debug/grunner
```

**Release build:**
```bash
cargo build --release
# Binary: target/release/grunner
```

**Optimized build with native CPU instructions:**
```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

**Installation script:**
```bash
./build.sh
# Installs to ~/.local/bin/grunner
```

### Cross-Compilation

For cross-compiling to other architectures:

**Install cross-compilation toolchain:**
```bash
# For ARM64 (Raspberry Pi, etc.)
rustup target add aarch64-unknown-linux-gnu
sudo apt install gcc-aarch64-linux-gnu
```

**Build for target:**
```bash
cargo build --release --target=aarch64-unknown-linux-gnu
```

## Code Organization

### Module Structure

```
src/
├── main.rs              # Application entry point
├── ui.rs               # GTK UI construction
├── list_model.rs       # Central search engine
├── config.rs           # Configuration management
├── launcher.rs         # Application scanning
├── search_provider.rs  # GNOME Shell integration
├── actions.rs          # System actions
├── app_item.rs         # Application result wrapper
├── cmd_item.rs         # Command result wrapper
├── obsidian_item.rs    # Obsidian action wrapper
├── obsidian_bar.rs     # Obsidian UI component
├── power_bar.rs        # Power management UI
├── search_result_item.rs # Search provider result wrapper
├── app_mode.rs         # Mode enumeration
├── utils.rs           # Utility functions
└── style.css          # CSS stylesheet
```

### Key Design Patterns

1. **Model-View-Controller (MVC):**
   - Model: `list_model.rs`, `launcher.rs`
   - View: `ui.rs`, CSS styling
   - Controller: Signal handlers in `ui.rs`

2. **Dependency Injection:**
   - Configuration passed via `Arc<Config>`
   - Search backends initialized with config

3. **Async/Await Pattern:**
   - Non-blocking I/O for commands and D-Bus
   - Tokio runtime for async operations

4. **Builder Pattern:**
   - GTK widget construction with builder methods
   - Configuration structs with default values

### Data Flow

**Application Launch:**
```
User Input → UI Signal → ListModel → Launcher → Actions → System
```

**Configuration:**
```
Config File → config::load() → Config Struct → Modules
```

**Search Results:**
```
Backend → Result Type → GObject Wrapper → ListStore → ListView
```

## Testing

### Unit Tests

**Running tests:**
```bash
# Run all tests
cargo test

# Run specific module tests
cargo test --test config

# Run with verbose output
cargo test -- --nocapture

# Run tests in parallel
cargo test -- --test-threads=4
```

**Test Organization:**
- Unit tests in same file as implementation (`mod tests`)
- Integration tests in `tests/` directory
- Mock external dependencies where possible

**Example Test:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loading() {
        let config = Config::default();
        assert_eq!(config.window.width, DEFAULT_WINDOW_WIDTH);
        assert_eq!(config.window.height, DEFAULT_WINDOW_HEIGHT);
    }

    #[test]
    fn test_config_loading() {
        let config = Config::default();
        assert_eq!(config.window.width, DEFAULT_WINDOW_WIDTH);
        assert_eq!(config.window.height, DEFAULT_WINDOW_HEIGHT);
    }
}
```

### Integration Tests

**Test Directory Structure:**
```
tests/
├── integration/
│   ├── config_tests.rs
│   ├── search_tests.rs
│   └── ui_tests.rs
└── helpers.rs
```

**Running Integration Tests:**
```bash
cargo test --test integration
```

### UI Testing

**Manual Testing Checklist:**
- [ ] Application launches correctly
- [ ] Search returns relevant results
- [ ] Keyboard navigation works
- [ ] Theme adaptation functions
- [ ] All colon commands work
- [ ] Obsidian integration functions
- [ ] Power actions work (with confirmation)
- [ ] Configuration changes take effect

**Automated UI Testing:**
```bash
# Requires gtk-rs test utilities
cargo add gtk-test --dev
```

### Performance Testing

**Benchmarks:**
```rust
#[bench]
fn bench_application_search(b: &mut Bencher) {
    let launcher = Launcher::new(&default_search_config()).unwrap();
    b.iter(|| launcher.search("term", 10));
}
```

**Running Benchmarks:**
```bash
cargo bench
```

## Debugging

### Logging

**Environment Variables:**
```bash
# Enable GTK debug logging
export G_MESSAGES_DEBUG=all

# Enable Rust logging
export RUST_LOG=debug

# Run with logging enabled
RUST_LOG=debug cargo run
```

**Custom Logging Macros:**
```rust
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            eprintln!($($arg)*);
        }
    };
}
```

### Debug Builds

**Debug Symbols:**
```bash
# Build with debug symbols
cargo build

# Run under debugger
gdb target/debug/grunner
```

**Debugging with VS Code:**
1. Install CodeLLDB extension
2. Create `.vscode/launch.json`:
```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug Grunner",
            "program": "${workspaceFolder}/target/debug/grunner",
            "args": [],
            "cwd": "${workspaceFolder}"
        }
    ]
}
```

### Common Issues and Solutions

**GTK/Libadwaita Issues:**
```bash
# Check GTK theme
export GTK_THEME=Adwaita

# Check icon theme
export GTK_ICON_THEME=Adwaita

# Reset GTK settings
rm -rf ~/.config/gtk-4.0
```

**Missing Dependencies:**
```bash
# Check for missing libraries
ldd target/release/grunner

# Install missing development packages
sudo apt-file search <missing-library>
```

## Performance Profiling

### Profiling Tools

**CPU Profiling:**
```bash
# Install perf
sudo apt install linux-tools-common linux-tools-generic

# Profile with perf
perf record -g target/release/grunner
perf report
```

**Memory Profiling:**
```bash
# Install valgrind
sudo apt install valgrind

# Memory check
valgrind --leak-check=full target/debug/grunner

# Heap profiling
valgrind --tool=massif target/debug/grunner
ms_print massif.out.*
```

**Flame Graphs:**
```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph
sudo flamegraph target/release/grunner
```

### Performance Metrics

**Key Metrics to Monitor:**
- Startup time (target: < 100ms)
- Search latency (target: < 50ms)
- Memory usage (target: < 100MB)
- CPU usage during idle (target: < 1%)

**Measurement Script:**
```bash
#!/bin/bash
# measure_performance.sh

echo "=== Grunner Performance Test ==="

# Startup time
echo -n "Startup time: "
time (timeout 0.5 target/release/grunner &>/dev/null) 2>&1 | grep real

# Memory usage
echo -n "Memory usage: "
/usr/bin/time -v target/release/grunner --version 2>&1 | grep "Maximum resident set size"
```

### Optimization Techniques

**Code Optimizations:**
1. **Caching:** Application list caching in `~/.cache/grunner/apps.bin`
2. **Lazy Loading:** Icons and resources loaded on-demand
3. **Async Operations:** Non-blocking I/O for commands and D-Bus
4. **String Interning:** Shared string references where possible
5. **Batch Processing:** Group similar operations

**Build Optimizations:**
1. **LTO (Link Time Optimization):** Enabled in release profile
2. **CPU-specific optimizations:** `-C target-cpu=native`
3. **Strip symbols:** Reduces binary size
4. **Panic abort:** Reduces binary size

## Code Style Guidelines

### Rust Conventions

**Formatting:**
```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check
```

**Linting:**
```bash
# Run clippy
cargo clippy -- -D warnings

# Clippy with all checks
cargo clippy --all-features -- -D warnings
```

### Naming Conventions

**Variables and Functions:**
- snake_case for variables and functions
- SCREAMING_SNAKE_CASE for constants
- CamelCase for types and traits
- Leading underscore for unused parameters

**File Naming:**
- snake_case.rs for module files
- Descriptive names reflecting module purpose

### Documentation

**Code Documentation:**
```rust
/// Brief description of the item
///
/// Detailed explanation of the item's purpose, behavior,
/// parameters, return values, and examples.
///
/// # Examples
/// ```
/// let result = function_name(arg1, arg2);
/// assert_eq!(result, expected);
/// ```
///
/// # Errors
/// Returns `Err` if something goes wrong.
///
/// # Panics
/// Panics under certain conditions.
pub fn function_name(param1: Type1, param2: Type2) -> Result<ReturnType, ErrorType> {
    // implementation
}
```

**Module Documentation:**
```rust
//! Module-level documentation
//!
//! Overview of the module's purpose, key types,
//! and important functions.
```

### Error Handling

**Result Types:**
```rust
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// Or custom error type
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

**Error Propagation:**
```rust
fn process_file(path: &str) -> Result<Data> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Io(e))?;
    
    // Additional processing
    Ok(parse_content(&content)?)
}
```

## Contributing

### Workflow

1. **Fork the repository**
2. **Create a feature branch:**
   ```bash
   git checkout -b feature/description
   ```
3. **Make changes and commit:**
   ```bash
   git add .
   git commit -m "feat: add new feature"
   ```
4. **Push to your fork:**
   ```bash
   git push origin feature/description
   ```
5. **Create a Pull Request**

### Commit Message Convention

Follow [Conventional Commits](https://www.conventionalcommits.org/):

**Format:** `type(scope): description`

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

**Examples:**
```
feat(search): add fuzzy matching for applications
fix(ui): correct window sizing on HiDPI displays
docs(config): update configuration examples
refactor(launcher): improve application caching
```

### Pull Request Guidelines

**Checklist for PRs:**
- [ ] Code follows style guidelines
- [ ] Tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated
- [ ] Commit messages follow convention
- [ ] Changes are focused and atomic

**PR Template:**
```markdown
## Description
Brief description of the changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
Describe how you tested the changes

## Screenshots (if UI changes)
Attach screenshots if applicable

## Checklist
- [ ] My code follows the style guidelines
- [ ] I have performed a self-review
- [ ] I have commented my code
- [ ] I have updated documentation
- [ ] My changes generate no new warnings
```

### Issue Reporting

**Bug Report Template:**
```markdown
## Description
Clear description of the bug

## Steps to Reproduce
1. Step 1
2. Step 2
3. Step 3

## Expected Behavior
What should happen

## Actual Behavior
What actually happens

## Environment
- OS: [e.g., Ubuntu 22.04]
- Grunner Version: [e.g., 0.7.0]
- GTK Version: [e.g., 4.10]
- Rust Version: [e.g., 1.75]

## Additional Context
Screenshots, logs, etc.
```

## Release Process

### Versioning

Follow [Semantic Versioning](https://semver.org/):
- **MAJOR**: Incompatible API changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

### Release Checklist

**Pre-release:**
1. [ ] Update version in `Cargo.toml`
2. [ ] Update CHANGELOG.md
3. [ ] Run full test suite
4. [ ] Update documentation
5. [ ] Verify dependencies are up to date

**Release:**
1. [ ] Create release tag: `git tag v0.7.0`
2. [ ] Push tag: `git push origin v0.7.0`
3. [ ] Create GitHub release