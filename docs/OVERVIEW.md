# Grunner - Technical Overview

## Project Summary

**Grunner** is a fast, keyboard-driven application launcher for GNOME and other Linux desktops, written in Rust. Inspired by Rofi, Grunner is built on GTK4 and libadwaita, and follows your system's light/dark theme and accent color automatically.

## Core Philosophy

Grunner is designed with several key principles in mind:

1. **Keyboard-first interaction**: All primary functionality is accessible via keyboard shortcuts
2. **System integration**: Deep integration with GNOME Shell, desktop standards, and system tools
3. **Extensibility**: Modular architecture supporting custom commands and plugins
4. **Performance**: Written in Rust for speed and memory safety
5. **Modern UI**: Built on GTK4/libadwaita with adaptive theming

## Architecture Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    GTK4/libadwaita UI Layer                  │
├─────────────────────────────────────────────────────────────┤
│                    Application Logic Layer                   │
├─────────────────────────────────────────────────────────────┤
│                    Data Access Layer                         │
└─────────────────────────────────────────────────────────────┘
```

### Component Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         main.rs                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    Application                      │   │
│  │  • GTK Application lifecycle                        │   │
│  │  • Configuration loading                            │   │
│  │  • Signal handling                                  │   │
│  └─────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                         ui.rs                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    UI Components                    │   │
│  │  • Window construction                             │   │
│  │  • Search entry widget                             │   │
│  │  • Results list view                               │   │
│  │  • Obsidian action bar                             │   │
│  │  • Power bar                                       │   │
│  └─────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                     list_model.rs                         │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    Search Engine                    │   │
│  │  • Query routing                                   │   │
│  │  • Mode switching                                  │   │
│  │  • Result population                               │   │
│  │  • Fuzzy matching                                  │   │
│  └─────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                    Specialized Modules                     │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐        │
│  │launcher │ │calculator│ │search   │ │actions │        │
│  │         │ │         │ │provider │ │        │        │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘        │
└─────────────────────────────────────────────────────────────┘
```

## Key Features

### 1. Application Launcher
- **Fuzzy search** across all installed `.desktop` applications
- **Icon and description display** for visual identification
- **Terminal application support** with auto-detected terminal emulators
- **Duplicate detection** for applications with multiple `.desktop` files

### 2. Search Modes
- **Default mode**: Application fuzzy search
- **Calculator mode**: Inline arithmetic expression evaluation
- **Colon commands**: Extensible command system with custom shell commands
- **File search**: Integration with `plocate` for fast file finding
- **Content search**: Integration with `ripgrep` for full-text search
- **GNOME Shell search**: Integration with GNOME Shell search providers
- **Obsidian integration**: Vault management and note operations

### 3. System Integration
- **Power management**: Suspend, restart, power off, logout
- **Theme adaptation**: Automatic light/dark mode and accent color following
- **D-Bus integration**: GNOME Shell search provider communication
- **XDG standards**: Configuration and data directory compliance

### 4. Configuration System
- **TOML-based configuration** with sensible defaults
- **Automatic config creation** on first run
- **Home directory expansion** (`~` support)
- **Runtime configurable** without recompilation

## Technology Stack

### Core Dependencies
- **Rust**: Primary programming language (edition 2024)
- **GTK4**: GUI toolkit for modern Linux applications
- **libadwaita**: GNOME HIG-compliant widgets and theming
- **glib**: Low-level system library bindings

### Search and Matching
- **fuzzy-matcher**: Fuzzy string matching algorithm
- **regex**: Regular expression support for advanced search
- **skim**: Fuzzy matching library (indirect dependency)

### Data Processing
- **serde**: Serialization/deserialization framework
- **toml**: TOML configuration parsing
- **chrono**: Date and time handling
- **rayon**: Data parallelism

### System Integration
- **zbus**: D-Bus communication for GNOME Shell integration
- **tokio**: Asynchronous runtime for concurrent operations
- **urlencoding**: URL encoding for Obsidian URI generation

### Evaluation
- **evalexpr**: Arithmetic expression evaluation for calculator mode

## Project Structure

### Source Code Organization
```
src/
├── main.rs              # Application entry point
├── ui.rs               # GTK UI construction and management
├── list_model.rs       # Central search model and query routing
├── config.rs           # Configuration loading and management
├── launcher.rs         # Application scanning and indexing
├── calculator.rs       # Arithmetic expression evaluation
├── search_provider.rs  # GNOME Shell search provider integration
├── actions.rs          # System actions (launch, open, power)
├── app_item.rs         # Application entry GObject wrapper
├── calc_item.rs        # Calculator result GObject wrapper
├── cmd_item.rs         # Command output GObject wrapper
├── obsidian_item.rs    # Obsidian action GObject wrapper
├── obsidian_bar.rs     # Obsidian action bar UI
├── power_bar.rs        # Power management UI
├── search_result_item.rs # Search provider result GObject wrapper
├── app_mode.rs         # Application mode enumeration
├── utils.rs            # Utility functions
└── style.css           # Application stylesheet
```

### Asset Structure
```
grunner/
├── assets/             # Application icons and graphics
├── screenshots/        # Documentation screenshots
├── src/               # Source code
├── target/            # Build artifacts (generated)
├── docs/              # Documentation (this directory)
├── Cargo.toml         # Rust project configuration
├── Cargo.lock         # Dependency lock file
├── README.md          # User documentation
├── LICENSE            # MIT License
├── build.sh           # Installation script
└── .gitignore         # Git ignore patterns
```

## Build System

### Cargo Configuration
- **Edition**: 2024
- **Optimization**: LTO enabled, panic=abort for release builds
- **Dependencies**: Managed via Cargo with version pinning

### Build Profiles
- **Debug**: Development builds with debugging symbols
- **Release**: Optimized builds with LTO and strip enabled

### Installation
- **Manual**: `cargo build --release`
- **Script**: `./build.sh` for automated installation to `~/.local/bin`
- **Desktop integration**: Automatic `.desktop` file generation

## Configuration Management

### Configuration File Location
```
~/.config/grunner/grunner.toml
```

### Configuration Sections
1. **Window**: Dimensions and display settings
2. **Search**: Result limits, debounce timing, application directories
3. **Calculator**: Inline calculator enable/disable
4. **Commands**: Custom shell commands for colon commands
5. **Obsidian**: Vault paths and note management settings

### Default Values
All configuration values have sensible defaults that work out-of-the-box on most Linux distributions with GNOME.

## Search Architecture

### Query Processing Pipeline
```
User Input → Mode Detection → Query Routing → Backend Execution → Result Processing → UI Display
```

### Mode Detection Logic
1. Check for colon prefix (`:`) → Command mode
2. Check for arithmetic expression → Calculator mode
3. Default → Application search mode

### Result Ranking
- **Application search**: Fuzzy match score based on name and description
- **File search**: `plocate` relevance scoring
- **Content search**: `ripgrep` match quality
- **Search providers**: GNOME Shell provider ranking

## UI Architecture

### GTK Widget Hierarchy
```
ApplicationWindow
├── Box (vertical)
│   ├── SearchEntry
│   ├── ScrolledWindow
│   │   └── ListView
│   │       └── ListItem
│   │           └── Box (horizontal)
│   │               ├── Image (icon)
│   │               └── Box (vertical)
│   │                   ├── Label (title)
│   │                   └── Label (subtitle)
│   └── Box (horizontal, bottom bar)
│       ├── Button (settings)
│       ├── Box (Obsidian actions, conditional)
│       └── Box (power actions, conditional)
```

### Theming System
- **CSS-based styling** embedded in binary
- **libadwaita variables** for system theme adaptation
- **Accent color support** via CSS custom properties
- **Responsive design** for different window sizes

## Data Flow

### Application Launch Flow
```
1. User types query
2. list_model processes query
3. launcher module searches .desktop files
4. Results filtered and ranked
5. UI updates with results
6. User selects result
7. actions module executes launch command
8. Application starts via g_spawn_async
```

### Command Execution Flow
```
1. User types :command argument
2. list_model detects command mode
3. Shell command executed asynchronously
4. Output parsed line by line
5. Results wrapped in cmd_item objects
6. UI displays command results
7. User can open/copy results
```

## Error Handling

### Error Categories
1. **Configuration errors**: Invalid TOML, missing files, permission issues
2. **Runtime errors**: Command execution failures, D-Bus communication errors
3. **UI errors**: Widget creation failures, signal handler errors
4. **System errors**: Missing dependencies, unsupported features

### Error Recovery
- **Graceful degradation**: Features disabled rather than crashing
- **User feedback**: Error messages in UI where appropriate
- **Logging**: Debug information for troubleshooting
- **Default fallbacks**: Sensible defaults when configuration is invalid

## Performance Considerations

### Optimizations Implemented
1. **Async I/O**: Non-blocking command execution and file operations
2. **Caching**: Application list caching between runs
3. **Lazy loading**: Resources loaded on-demand
4. **Debounced search**: Reduced UI updates during typing
5. **Parallel processing**: Concurrent search provider queries

### Memory Management
- **Rust ownership system**: Compile-time memory safety
- **GObject reference counting**: GTK memory management
- **Efficient data structures**: Minimized allocations and copies
- **Resource cleanup**: Proper disposal of GTK objects

## Security Considerations

### Security Model
1. **No elevated privileges**: Runs with user permissions only
2. **Shell command sanitization**: Argument escaping for custom commands
3. **File path validation**: Prevention of path traversal attacks
4. **D-Bus method validation**: Restricted to safe operations

### Privacy Features
1. **Local processing**: No network communication or telemetry
2. **Configurable search scope**: User controls what directories are indexed
3. **Transparent operations**: All commands visible and configurable

## Extension Points

### Customization Options
1. **Custom commands**: Shell commands via configuration
2. **CSS theming**: Style customization via recompilation
3. **Application directories**: Configurable .desktop file locations
4. **Obsidian integration**: Vault and note management configuration

### Potential Extensions
1. **Plugin system**: Dynamic loading of Rust modules
2. **Additional search backends**: Web search, database search, etc.
3. **Advanced theming**: CSS variable customization
4. **Keyboard shortcut customization**: User-defined key bindings

## Platform Support

### Officially Supported
- **GNOME 40+** with Wayland or X11
- **Systemd-based distributions** for power management
- **Linux distributions** with GTK4 and libadwaita packages

### Community Tested
- **Fedora** 36+
- **Ubuntu** 22.04+
- **Arch Linux** with latest packages
- **Debian** 12+

### Requirements
- **Rust** 1.70+
- **GTK4** 4.6+
- **libadwaita** 1.6+
- **plocate** (optional, for file search)
- **ripgrep** (optional, for content search)
- **Obsidian** (optional, for vault integration)

## Development Workflow

### Getting Started
1. Clone repository
2. Install Rust and dependencies
3. `cargo build` for development
4. `cargo run` to test changes

### Testing
- **Unit tests**: `cargo test`
- **Integration testing**: Manual UI testing
- **Performance testing**: Benchmarking search operations

### Contribution Guidelines
1. Follow Rust coding conventions
2. Add documentation for new features
3. Update configuration defaults if needed
4. Test on multiple distributions if possible

## Future Development

### Roadmap Items
1. **Plugin API** for third-party extensions
2. **Advanced search syntax** (AND/OR operators, filters)
3. **Session management** (save/restore window state)
4. **Multi-monitor support** (window placement options)
5. **Advanced theming** (color scheme customization)

### Technical Debt
1. **Error handling unification** across modules
2. **Test coverage improvement** for UI components
3. **Documentation expansion** for internal APIs
4. **Performance profiling** and optimization

## Conclusion

Grunner represents a modern approach to application launching on Linux desktops, combining the performance benefits of Rust with the polished UI of GTK4/libadwaita. Its modular architecture, extensive configuration options, and deep system integration make it a powerful tool for power users while remaining accessible to beginners through sensible defaults and intuitive design.

The project demonstrates best practices in Rust GUI development, including proper error handling, async I/O, memory safety, and system integration while maintaining a clean, maintainable codebase organized around clear separation of concerns.