# Grunner Architecture Documentation

## Table of Contents
1. [System Architecture](#system-architecture)
2. [Module Dependencies](#module-dependencies)
3. [Data Flow](#data-flow)
4. [UI Architecture](#ui-architecture)
5. [Search System](#search-system)
6. [Configuration System](#configuration-system)
7. [Error Handling](#error-handling)
8. [Performance Considerations](#performance-considerations)

## System Architecture

### Overall Architecture
Grunner follows a layered architecture with clear separation between UI, business logic, and data access:

```
┌─────────────────────────────────────────────────────────────┐
│                    Presentation Layer                       │
│  • GTK4/libadwaita widgets                                  │
│  • CSS styling                                              │
│  • User input handling                                      │
│  • Result display                                           │
├─────────────────────────────────────────────────────────────┤
│                    Application Layer                        │
│  • Search logic and routing                                 │
│  • Mode management                                          │
│  • Command execution                                        │
│  • Action handling                                          │
├─────────────────────────────────────────────────────────────┤
│                    Data Access Layer                        │
│  • .desktop file parsing                                    │
│  • File system access                                       │
│  • D-Bus communication                                      │
│  • Configuration loading                                    │
└─────────────────────────────────────────────────────────────┘
```

### Component Diagram
```
┌─────────────────────────────────────────────────────────────┐
│                         main.rs                             │
│  Entry point, app lifecycle, config loading                 │
├─────────────────────────────────────────────────────────────┤
│                         ui.rs                               │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐            │
│  │   Window    │ │ SearchEntry │ │ ListView    │            │
│  │             │ │             │ │             │            │
│  └─────────────┘ └─────────────┘ └─────────────┘            │
│  ┌─────────────┐ ┌─────────────┐                            │
│  │ ObsidianBar │ │ PowerBar    │                            │
│  │             │ │             │                            │
│  └─────────────┘ └─────────────┘                            │
├─────────────────────────────────────────────────────────────┤
│                     list_model.rs                           │
│  Central dispatcher, mode switching, result aggregation     │
├─────────────────────────────────────────────────────────────┤
│                    Specialized Modules                      │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐                        │
│  │launcher │ │search   │ │actions  │                         │
│  │         │ │provider │ │         │                         │
│  └─────────┘ └─────────┘ └─────────┘                        │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐                        │
│  │config   │ │utils    │ │app_mode │                        │
│  │         │ │         │ │         │                        │
│  └─────────┘ └─────────┘ └─────────┘                        │
└─────────────────────────────────────────────────────────────┘
```

## Module Dependencies

### Core Module Relationships
```
main.rs
├── config.rs (loads configuration)
├── ui.rs (builds UI with config)
└── list_model.rs (search engine with config)

ui.rs
├── list_model.rs (search results)
├── obsidian_bar.rs (Obsidian actions)
├── power_bar.rs (power management)
└── actions.rs (action execution)

list_model.rs
├── launcher.rs (application search)
├── search_provider.rs (GNOME search)
├── actions.rs (command execution)
└── app_mode.rs (mode enumeration)

actions.rs
├── utils.rs (path expansion, shell escaping)
└── (various system dependencies)
```

### Detailed Module Descriptions

#### main.rs
- **Purpose**: Application entry point and lifecycle management
- **Responsibilities**:
  - Parse command-line arguments
  - Load configuration from file
  - Create GTK Application instance
  - Connect activation signal to UI builder
  - Handle application shutdown
- **Key Functions**:
  - `main()`: Entry point with error handling
  - Configuration loading via `config::load()`
  - GTK application initialization

#### ui.rs
- **Purpose**: Construct and manage the GTK user interface
- **Responsibilities**:
  - Build window hierarchy with GTK4 widgets
  - Create search entry with debounced input
  - Set up list view for results display
  - Manage Obsidian and power action bars
  - Handle keyboard navigation
  - Apply CSS styling
- **Key Structures**:
  - `build_ui()`: Main UI construction function
  - Signal handlers for user interaction
  - Widget reference management

#### list_model.rs
- **Purpose**: Central search engine and query dispatcher
- **Responsibilities**:
  - Detect search mode based on input
  - Route queries to appropriate backend
  - Aggregate and rank results
  - Manage search state and history
  - Coordinate async operations
- **Key Algorithms**:
  - Mode detection (command vs app search)
  - Result ranking and filtering
  - Async command execution with debouncing

#### config.rs
- **Purpose**: Configuration management and validation
- **Responsibilities**:
  - Load TOML configuration from file
  - Provide default values for missing settings
  - Expand home directory paths (`~`)
  - Validate configuration integrity
- **Key Structures**:
  - `Config`: Main configuration struct
  - `WindowConfig`, `SearchConfig`, `ObsidianConfig`: Section structs
  - `load()`: Public API for configuration loading

#### launcher.rs
- **Purpose**: Application discovery and indexing
- **Responsibilities**:
  - Scan `.desktop` files from configured directories
  - Parse desktop entry files
  - Deduplicate applications
  - Cache application list for performance
  - Provide fuzzy search capabilities
- **Key Algorithms**:
  - Desktop entry parsing with fallbacks
  - Application deduplication by executable path
  - Fuzzy matching with `fuzzy-matcher` crate



#### search_provider.rs
- **Purpose**: GNOME Shell search provider integration
- **Responsibilities**:
  - Discover available search providers via D-Bus
  - Query providers asynchronously
  - Merge results from multiple providers
  - Activate search results via D-Bus
- **Key Technologies**:
  - `zbus` for D-Bus communication
  - `tokio` for async operations
  - GNOME Shell search provider API

#### actions.rs
- **Purpose**: System action execution
- **Responsibilities**:
  - Launch applications and files
  - Execute shell commands
  - Handle power management operations
  - Open Obsidian notes and vaults
  - Copy text to clipboard
- **Key Functions**:
  - `launch_app()`: Application launching with terminal detection
  - `open_file()`: File opening with `xdg-open` or `$EDITOR`
  - `execute_power_action()`: System power management

## Data Flow

### Application Launch Sequence
```
1. User types in search entry
   ↓
2. ui.rs: Input signal triggers search
   ↓
3. list_model.rs: Process query, detect mode
   ↓
4. launcher.rs: Fuzzy search .desktop files
   ↓
5. list_model.rs: Rank and filter results
   ↓
6. ui.rs: Update list view with results
   ↓
7. User selects result, presses Enter
   ↓
8. actions.rs: Execute launch command
   ↓
9. System: Application starts
```

### Command Execution Sequence
```
1. User types ":command argument"
   ↓
2. list_model.rs: Detect command mode
   ↓
3. config.rs: Look up shell command
   ↓
4. actions.rs: Execute command asynchronously
   ↓
5. Parse stdout line by line
   ↓
6. Wrap each line in cmd_item
   ↓
7. ui.rs: Display results in list
   ↓
8. User can open/copy results
```

### Configuration Loading Sequence
```
1. Application starts
   ↓
2. main.rs: Call config::load()
   ↓
3. config.rs: Check ~/.config/grunner/grunner.toml
   ↓
4. If exists: parse TOML, merge with defaults
   ↓
5. If not exists: create with defaults
   ↓
6. Expand home directories in paths
   ↓
7. Validate configuration values
   ↓
8. Return Config struct to main.rs
```

## UI Architecture

### Widget Hierarchy
```
ApplicationWindow
├── AdwWindow (libadwaita window)
│   ├── Box (vertical, main container)
│   │   ├── SearchEntry (GTK4 search entry)
│   │   │   ├── Icon (search icon)
│   │   │   └── Text input area
│   │   ├── ScrolledWindow (results area)
│   │   │   └── ListView (GTK4 list view)
│   │   │       └── ListItem factory
│   │   │           └── Box (horizontal, per result)
│   │   │               ├── Image (application icon)
│   │   │               └── Box (vertical, text)
│   │   │                   ├── Label (title)
│   │   │                   └── Label (subtitle)
│   │   └── Box (horizontal, bottom bar)
│   │       ├── Button (settings)
│   │       ├── Box (Obsidian actions, when :ob)
│   │       │   ├── Button (Open Vault)
│   │       │   ├── Button (New Note)
│   │       │   ├── Button (Daily Note)
│   │       │   └── Button (Quick Note)
│   │       └── Box (power actions, when idle)
│   │           ├── Button (Suspend)
│   │           ├── Button (Restart)
│   │           ├── Button (Power Off)
│   │           └── Button (Log Out)
└── StyleProvider (CSS styling)
```

### Signal Connections
```
SearchEntry::changed → list_model::search() → UI update
ListView::activate → actions::execute_action() → System action
KeyPress events → UI navigation handlers
Configuration changes → UI refresh
```

### CSS Styling System
- **Embedded CSS**: `style.css` compiled into binary
- **libadwaita Variables**: Uses `var(--accent-color)`, `var(--window-bg-color)`
- **Responsive Design**: Adapts to window size changes
- **Theme Support**: Automatic light/dark mode switching

## Search System

### Mode Detection Algorithm
```rust
fn detect_mode(query: &str) -> AppMode {
    if query.starts_with(':') {
        // Parse command and argument
        let parts: Vec<&str> = query[1..].splitn(2, ' ').collect();
        if parts.is_empty() {
            return AppMode::Command("".into(), None);
        }
        let cmd = parts[0];
        let arg = parts.get(1).map(|s| (*s).to_string());
        return AppMode::Command(cmd.into(), arg);
    }
    
    AppMode::AppSearch(query.into())
}
```

### Search Backends

#### Application Search (launcher.rs)
- **Data Source**: `.desktop` files from XDG directories
- **Matching**: Fuzzy matching with `fuzzy-matcher`
- **Ranking**: Match score based on name and description
- **Caching**: Application list cached between searches

#### File Search (plocate integration)
- **Command**: `plocate -i -- "$1" 2>/dev/null`
- **Filtering**: Limited to home directory
- **Results**: 20 most relevant files
- **Opening**: `xdg-open` or `$EDITOR` for text files

#### Content Search (ripgrep integration)
- **Command**: `rg --with-filename --line-number --no-heading -S "$1" ~`
- **Scope**: Recursive home directory search
- **Format**: `file:line:content` display
- **Opening**: `$EDITOR` at specific line

#### GNOME Shell Search
- **Protocol**: D-Bus search provider API
- **Discovery**: System bus, `org.gnome.Shell.SearchProvider2`
- **Query**: Async queries to all providers
- **Activation**: D-Bus method calls

### Result Ranking and Filtering
1. **Score Calculation**: Fuzzy match score (0-100)
2. **Type Boosting**: Application names weighted higher than descriptions
3. **Relevance Filtering**: Minimum score threshold
4. **Limit Enforcement**: Configurable max results (default: 64)
5. **Deduplication**: Remove duplicate applications

## Configuration System

### Configuration File Structure
```toml
# ~/.config/grunner/grunner.toml

[window]
width = 640
height = 480

[search]
max_results = 64
command_debounce_ms = 300
app_dirs = [
    "/usr/share/applications",
    "~/.local/share/applications",
]

[commands]
f = "plocate -i -- \"$1\" 2>/dev/null | grep \"^$HOME/\" | head -20"
fg = "rg --with-filename --line-number --no-heading -S \"$1\" ~ 2>/dev/null | head -20"

[obsidian]
vault = "~/Documents/Obsidian/MyVault"
daily_notes_folder = "Daily"
new_notes_folder = "Inbox"
quick_note = "Quick.md"
```

### Configuration Loading Process
1. **Path Resolution**: `~/.config/grunner/grunner.toml`
2. **File Existence Check**: Create default if missing
3. **TOML Parsing**: `toml::from_str()` with error handling
4. **Default Merging**: `Config::merge()` for missing values
5. **Path Expansion**: `~` to home directory expansion
6. **Validation**: Type checking and value ranges

### Default Values Management
- **Built-in Defaults**: Defined as constants in `config.rs`
- **Fallback Chain**: File → Environment → Built-in defaults
- **Validation**: Range checks for numeric values
- **Path Verification**: Directory existence checking (non-fatal)

## Error Handling

### Error Types
1. **Configuration Errors**:
   - Invalid TOML syntax
   - Missing required files
   - Permission denied
   - Invalid value types

2. **Runtime Errors**:
   - Command execution failures
   - D-Bus communication errors
   - File system errors
   - Network errors (if applicable)

3. **UI Errors**:
   - Widget creation failures
   - Signal connection errors
   - CSS parsing errors
   - Memory allocation errors

### Error Handling Strategy
- **Graceful Degradation**: Disable features rather than crash
- **User Feedback**: Display errors in UI where appropriate
- **Logging**: Debug information for troubleshooting
- **Default Fallbacks**: Sensible defaults when configuration fails

### Error Recovery
```rust
fn load_config() -> Config {
    match std::fs::read_to_string(config_path()) {
        Ok(content) => {
            match toml::from_str(&content) {
                Ok(mut config) => {
                    config.merge_defaults();
                    config
                }
                Err(e) => {
                    eprintln!("Config parse error: {}, using defaults", e);
                    Config::default()
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Create default config
            save_config(&Config::default());
            Config::default()
        }
        Err(e) => {
            eprintln!("Config read error: {}, using defaults", e);
            Config::default()
        }
    }
}
```

## Performance Considerations

### Optimizations Implemented

#### 1. Application List Caching
- **Cache Location**: `~/.cache/grunner/apps.bin`
- **Serialization**: `bincode` for fast binary serialization
- **Invalidation**: Cache invalidated on directory changes
- **Fallback**: Live scanning if cache fails

#### 2. Async Operations
- **Command Execution**: Non-blocking shell command execution
- **D-Bus Calls**: Async D-Bus method calls
- **File I/O**: Async file reading where possible
- **Parallel Processing**: Concurrent search provider queries

#### 3. UI Performance
- **Debounced Search**: 300ms debounce for typing
- **Virtual Scrolling**: GTK4 ListView virtual scrolling
- **Lazy Loading**: Icons loaded on-demand
- **CSS Optimization**: Minimal CSS selectors

#### 4. Memory Management
- **String Interning**: Shared string references where possible
- **Object Pools**: Reuse of GObject instances
- **Reference Counting**: Proper GTK memory management
- **Early Freeing**: Release unused resources promptly

### Performance Metrics
- **Startup Time**: < 100ms with warm cache
- **Search Latency**: < 50ms for typical queries
- **Memory Usage**: ~50MB typical, ~100MB peak
- **CPU Usage**: < 1% idle, < 5% during search

### Profiling and Optimization
- **Benchmarking**: `cargo bench` for critical paths
- **Profiling**: `perf`, `flamegraph` for performance analysis
- **Memory Profiling**: `valgrind`, `heaptrack` for memory usage
- **Continuous Monitoring**: Performance regression testing

## Concurrency Model

### Async/Await Pattern
- **Runtime**: `tokio` for async operations
- **Task Spawning**: Lightweight tasks for independent operations
- **Error Propagation**: `Result` and `Option` types with async
- **Cancellation**: Task cancellation on new searches

### Thread Safety
