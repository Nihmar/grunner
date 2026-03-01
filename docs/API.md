# Grunner API Documentation

## Table of Contents
1. [Module Overview](#module-overview)
2. [Configuration API](#configuration-api)
3. [Search API](#search-api)
4. [UI API](#ui-api)
5. [Action API](#action-api)
6. [Integration APIs](#integration-apis)
7. [Utility APIs](#utility-apis)

## Module Overview

### Core Modules

#### main.rs
**Purpose**: Application entry point and lifecycle management.

**Key Functions**:
```rust
fn main() -> glib::ExitCode
```
- Entry point for the Grunner application
- Parses command-line arguments
- Loads configuration via `config::load()`
- Creates and runs GTK Application instance
- Returns `ExitCode::SUCCESS` on normal execution

**Constants**:
- `APP_ID: &str = "org.nihmar.grunner"` - Application ID for D-Bus and GNOME Shell integration

#### config.rs
**Purpose**: Configuration loading, parsing, and management.

**Key Structures**:
```rust
pub struct Config {
    pub window: WindowConfig,
    pub search: SearchConfig,
    pub calculator: CalculatorConfig,
    pub commands: HashMap<String, String>,
    pub obsidian: Option<ObsidianConfig>,
}

pub struct WindowConfig {
    pub width: i32,
    pub height: i32,
}

pub struct SearchConfig {
    pub max_results: usize,
    pub command_debounce_ms: u32,
    pub app_dirs: Vec<String>,
}

pub struct CalculatorConfig {
    pub enabled: bool,
}

pub struct ObsidianConfig {
    pub vault: String,
    pub daily_notes_folder: String,
    pub new_notes_folder: String,
    pub quick_note: String,
}
```

**Key Functions**:
```rust
pub fn load() -> Config
```
- Loads configuration from `~/.config/grunner/grunner.toml`
- Creates default configuration if file doesn't exist
- Merges user settings with built-in defaults
- Returns validated `Config` struct

```rust
pub fn default_app_dirs() -> Vec<String>
```
- Returns default list of application directories to scan
- Includes system-wide, user-local, and Flatpak directories

**Constants**:
- `DEFAULT_WINDOW_WIDTH: i32 = 640`
- `DEFAULT_WINDOW_HEIGHT: i32 = 480`
- `DEFAULT_MAX_RESULTS: usize = 64`
- `DEFAULT_CALCULATOR: bool = false`
- `DEFAULT_COMMAND_DEBOUNCE_MS: u32 = 300`

#### ui.rs
**Purpose**: GTK4 UI construction and management.

**Key Functions**:
```rust
pub fn build_ui(app: &Application, cfg: Arc<Config>) -> ApplicationWindow
```
- Constructs the main application window
- Creates search entry, results list, and action bars
- Sets up signal handlers and keyboard navigation
- Applies CSS styling from embedded stylesheet
- Returns the configured `ApplicationWindow`

**Widget Hierarchy**:
- `ApplicationWindow` (root window)
- `AdwWindow` (libadwaita window wrapper)
- `SearchEntry` (text input with search icon)
- `ListView` (virtual scrolling results list)
- `ObsidianBar` (Obsidian action buttons, conditional)
- `PowerBar` (system power actions, conditional)

#### list_model.rs
**Purpose**: Central search engine and query dispatcher.

**Key Structures**:
```rust
pub struct ListModel {
    config: Arc<Config>,
    launcher: Arc<Launcher>,
    current_mode: AppMode,
    // ... internal state
}

pub enum AppMode {
    AppSearch(String),
    Calculator(String),
    Command(String, Option<String>),
    Obsidian(String),
}
```

**Key Functions**:
```rust
pub fn new(config: Arc<Config>) -> Self
```
- Creates new ListModel instance
- Initializes search backends
- Sets up default state

```rust
pub async fn search(&self, query: String) -> Vec<glib::Object>
```
- Main search entry point
- Detects search mode based on query
- Routes to appropriate backend
- Returns list of GObject results for UI display

```rust
pub fn get_current_mode(&self) -> &AppMode
```
- Returns current search mode
- Used for UI state management

#### launcher.rs
**Purpose**: Application discovery, indexing, and fuzzy search.

**Key Structures**:
```rust
pub struct Launcher {
    apps: Vec<AppEntry>,
    cache_path: PathBuf,
}

pub struct AppEntry {
    pub name: String,
    pub exec: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub terminal: bool,
    pub categories: Vec<String>,
}
```

**Key Functions**:
```rust
pub fn new(config: &SearchConfig) -> Result<Self>
```
- Creates new Launcher instance
- Loads or builds application cache
- Returns `Result` for error handling

```rust
pub fn search(&self, query: &str, max_results: usize) -> Vec<&AppEntry>
```
- Performs fuzzy search on application entries
- Returns ranked results up to `max_results`
- Uses `fuzzy-matcher` crate for matching

```rust
pub fn get_all_apps(&self) -> &[AppEntry]
```
- Returns all loaded applications
- Used for debugging and testing

#### calculator.rs
**Purpose**: Arithmetic expression evaluation.

**Key Functions**:
```rust
pub fn evaluate(expression: &str) -> Result<f64, CalculatorError>
```
- Evaluates arithmetic expression
- Returns `Result<f64, CalculatorError>`
- Handles integer-to-float promotion
- Supports: `+ - * / % ^ ( )`

```rust
pub fn is_arithmetic_expression(text: &str) -> bool
```
- Checks if text appears to be an arithmetic expression
- Returns `bool` for mode detection

**Error Types**:
```rust
pub enum CalculatorError {
    ParseError(String),
    EvaluationError(String),
    DivisionByZero,
    InvalidExpression,
}
```

#### search_provider.rs
**Purpose**: GNOME Shell search provider integration.

**Key Structures**:
```rust
pub struct SearchProvider {
    connection: zbus::Connection,
    providers: Vec<ProviderInfo>,
}

pub struct ProviderInfo {
    pub bus_name: String,
    pub object_path: String,
    pub name: String,
    pub icon: String,
}
```

**Key Functions**:
```rust
pub async fn new() -> Result<Self>
```
- Creates new SearchProvider instance
- Discovers available providers via D-Bus
- Returns `Result` for error handling

```rust
pub async fn search(&self, query: &str, max_results: u32) -> Vec<SearchResult>
```
- Queries all discovered providers
- Returns merged and ranked results
- Uses async D-Bus calls via `zbus`

```rust
pub async fn activate_result(&self, provider: &ProviderInfo, identifier: &str) -> Result<()>
```
- Activates a search result via D-Bus
- Returns `Result` indicating success/failure

#### actions.rs
**Purpose**: System action execution.

**Key Functions**:
```rust
pub fn launch_app(app: &AppEntry) -> Result<()>
```
- Launches application from `.desktop` entry
- Handles terminal applications with auto-detection
- Returns `Result` for error handling

```rust
pub fn open_file(path: &str, line: Option<u32>) -> Result<()>
```
- Opens file with appropriate application
- Uses `xdg-open` for general files
- Uses `$EDITOR` for text files with line numbers
- Returns `Result` for error handling

```rust
pub fn execute_power_action(action: PowerAction) -> Result<()>
```
- Executes system power management actions
- Uses `systemctl` or `loginctl`
- Returns `Result` for error handling

```rust
pub fn copy_to_clipboard(text: &str) -> Result<()>
```
- Copies text to system clipboard
- Returns `Result` for error handling

**Enums**:
```rust
pub enum PowerAction {
    Suspend,
    Restart,
    PowerOff,
    Logout,
}
```

## Configuration API

### Configuration Loading

**Function Signature**:
```rust
pub fn load() -> Config
```

**Behavior**:
1. Checks for config file at `~/.config/grunner/grunner.toml`
2. If file exists, parses TOML and merges with defaults
3. If file doesn't exist, creates default config and saves it
4. Expands `~` in all path strings
5. Validates configuration values
6. Returns final `Config` struct

**Error Handling**:
- Invalid TOML: Falls back to defaults, logs error
- Permission denied: Falls back to defaults, logs error
- Missing file: Creates default config
- Invalid values: Uses defaults, logs warning

### Configuration Structure

**Window Configuration**:
```rust
WindowConfig {
    width: i32,    // Window width in pixels (default: 640)
    height: i32,   // Window height in pixels (default: 480)
}
```

**Search Configuration**:
```rust
SearchConfig {
    max_results: usize,          // Max results to display (default: 64)
    command_debounce_ms: u32,    // Debounce delay in ms (default: 300)
    app_dirs: Vec<String>,       // Directories to scan for .desktop files
}
```

**Calculator Configuration**:
```rust
CalculatorConfig {
    enabled: bool,  // Enable inline calculator (default: false)
}
```

**Obsidian Configuration**:
```rust
ObsidianConfig {
    vault: String,               // Path to Obsidian vault
    daily_notes_folder: String,  // Daily notes subfolder
    new_notes_folder: String,    // New notes subfolder
    quick_note: String,          // Quick note file path
}
```

**Commands Configuration**:
```rust
HashMap<String, String>  // Command name → shell command template
```

### Configuration Methods

**Config Struct Methods**:
```rust
impl Config {
    pub fn merge_defaults(&mut self)
    // Merges missing values with built-in defaults
    
    pub fn validate(&self) -> Result<(), ConfigError>
    // Validates configuration values
    
    pub fn save(&self) -> Result<(), std::io::Error>
    // Saves configuration to file
}
```

## Search API

### Search Mode Detection

**Function**: `detect_mode(query: &str) -> AppMode`

**Algorithm**:
1. If query starts with `:` → Command mode
   - Parse command and optional argument
   - Special handling for `:ob` (Obsidian) and `:s` (GNOME Shell)
2. Else if query matches arithmetic pattern → Calculator mode
3. Else → Application search mode

**Arithmetic Pattern**:
```regex
^[0-9\s\.\+\-\*\/\%\^\(\)]+$
```

### Search Backends

#### Application Search
**Backend**: `launcher::Launcher`
**Function**: `search(query: &str, max_results: usize) -> Vec<&AppEntry>`
**Matching**: Fuzzy matching with `fuzzy-matcher`
**Scoring**: Match score (0-100) based on name and description
**Ranking**: Higher scores first, with name matches weighted higher

#### Command Search
**Backend**: Shell command execution via `actions::execute_command()`
**Template**: `$1` replaced with user argument
**Output Parsing**: Line-by-line parsing, each line becomes a result
**Error Handling**: Silent failure (results empty on error)

#### Calculator Search
**Backend**: `calculator::evaluate()`
**Evaluation**: Safe expression evaluation via `evalexpr`
**Type Promotion**: Integer division promotes to float
**Error Display**: Shows error message as result on failure

#### GNOME Shell Search
**Backend**: `search_provider::SearchProvider`
**Protocol**: D-Bus `org.gnome.Shell.SearchProvider2`
**Query**: Async queries to all providers
**Merging**: Results from all providers combined and ranked

### Result Types

**Application Result** (`app_item.rs`):
```rust
pub struct AppItem {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub exec: String,
    pub terminal: bool,
}
```

**Command Result** (`cmd_item.rs`):
```rust
pub struct CmdItem {
    pub title: String,
    pub subtitle: String,
    pub raw_line: String,
}
```

**Calculator Result** (`calc_item.rs`):
```rust
pub struct CalcItem {
    pub expression: String,
    pub result: f64,
    pub error: Option<String>,
}
```

**Obsidian Result** (`obsidian_item.rs`):
```rust
pub struct ObsidianItem {
    pub action: ObsidianAction,
    pub text: Option<String>,
    pub path: Option<String>,
}
```

**Search Provider Result** (`search_result_item.rs`):
```rust
pub struct SearchResultItem {
    pub title: String,
    pub description: Option<String>,
    pub icon: String,
    pub provider: String,
    pub identifier: String,
}
```

## UI API

### Window Construction

**Function**: `build_ui(app: &Application, cfg: Arc<Config>) -> ApplicationWindow`

**Steps**:
1. Create `ApplicationWindow` with configured dimensions
2. Set up `SearchEntry` with debounced input handler
3. Create `ListView` with `ListStore` for results
4. Set up `ListItemFactory` for result rendering
5. Add `ObsidianBar` and `PowerBar` as needed
6. Apply CSS styling from embedded stylesheet
7. Connect keyboard navigation handlers
8. Return configured window

### Signal Handlers

**Search Entry**:
- `changed`: Triggers `list_model.search()` with debouncing
- `activate`: Activates selected result
- `key-press-event`: Handles arrow keys, Escape, Page Up/Down

**List View**:
- `activate`: Calls `actions::execute_action()` for selected item
- `selected-items-changed`: Updates UI state

**Buttons**:
- `clicked`: Executes corresponding action (Obsidian, power, settings)

### CSS Styling

**Embedded Stylesheet**: `src/style.css`
**Compilation**: Embedded in binary at build time
**Variables**: Uses libadwaita CSS custom properties:
- `var(--accent-color)`: System accent color
- `var(--window-bg-color)`: Window background
- `var(--text-color)`: Text color
- `var(--dim-label-color)`: Dimmed text color

**Key Selectors**:
- `.grunner-window`: Main window styling
- `.grunner-search`: Search entry styling
- `.grunner-list`: Results list styling
- `.grunner-item`: Individual result item
- `.grunner-bottom-bar`: Bottom action bar

## Action API

### Application Launching

**Function**: `launch_app(app: &AppEntry) -> Result<()>`

**Process**:
1. Check if application requires terminal
2. If terminal required:
   - Detect available terminal emulator
   - Construct command: `terminal -e "executable args"`
3. If no terminal required:
   - Use `g_spawn_async()` to launch executable
4. Handle `%` expansions from `.desktop` files
5. Return `Result` indicating success/failure

**Terminal Detection Order**:
1. `foot`
2. `alacritty`
3. `kitty`
4. `wezterm`
5. `ghostty`
6. `gnome-terminal`
7. `xfce4-terminal`
8. `konsole`
9. `xterm`

### File Operations

**Function**: `open_file(path: &str, line: Option<u32>) -> Result<()>`

**Process**:
1. Check file type via `file` command or extension
2. If text file and line specified:
   - Use `$EDITOR` environment variable
   - Format: `$EDITOR +line path`
3. If general file:
   - Use `xdg-open path`
4. Handle errors (file not found, no application)

**Environment Variables**:
- `$EDITOR`: Preferred text editor (default: `vim`)
- `$VISUAL`: Alternative editor variable

### Power Management

**Function**: `execute_power_action(action: PowerAction) -> Result<()>`

**Actions**:
- `Suspend`: `systemctl suspend` or `loginctl suspend`
- `Restart`: `systemctl reboot` or `loginctl reboot`
- `PowerOff`: `systemctl poweroff` or `loginctl poweroff`
- `Logout`: `loginctl terminate-user $UID`

**Confirmation**: Shows dialog before destructive actions
**Privileges**: May require polkit authorization

### Clipboard Operations

**Function**: `copy_to_clipboard(text: &str) -> Result<()>`

**Implementation**:
- Uses GTK4 clipboard API via `gtk4::Clipboard`
- Supports both primary and clipboard selections
- Returns `Result` for error handling

## Integration APIs

### GNOME Shell Search Provider API

**Interface**: `org.gnome.Shell.SearchProvider2`
**Methods**:
- `GetInitialResultSet(terms: Vec<String>) -> Vec<String>`
- `GetSubsearchResultSet(previous_results: Vec<String>, terms: Vec<String>) -> Vec<String>`
- `GetResultMetas(identifiers: Vec<String>) -> Vec<HashMap<String, Variant>>`
- `ActivateResult(identifier: &str, terms: Vec<String>, timestamp: u32)`

**Implementation**: `search_provider.rs`
**Async Support**: All methods are async via `zbus`

### Obsidian URI API

**URI Scheme**: `obsidian://`
**Actions**:
- Open vault: `obsidian://open?vault=Name`
- Open file: `obsidian://open?path=file.md&vault=Name`
- Create note: `obsidian://new?vault=Name&name=Note.md&content=...`
- Append to file: `obsidian://append?vault=Name&path=file.md&text=...`

**Implementation**: `actions::open_obsidian_uri()`
**Encoding**: URL encoding via `urlencoding` crate

### D-Bus Integration

**Crate**: `zbus`
**Connection**: System bus for search providers
**Async**: All D-Bus calls are async
**Error Handling**: Proper error types and logging

## Utility APIs

### Path Utilities (`utils.rs`)

**Functions**:
```rust
pub fn expand_home(path