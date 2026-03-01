//! Configuration management for Grunner
//!
//! This module handles loading, parsing, and providing access to the application's
//! configuration settings. It supports both built-in defaults and user-customizable
//! settings via a TOML configuration file.
//!
//! The configuration system provides:
//! - Window dimensions and UI settings
//! - Search behavior and result limits
//! - Application directory scanning paths
//! - Calculator functionality toggle
//! - Custom shell commands for search modes
//! - Obsidian vault integration settings
//! - Search provider filtering

use crate::utils::expand_home;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Default window width in pixels
pub const DEFAULT_WINDOW_WIDTH: i32 = 640;
/// Default window height in pixels
pub const DEFAULT_WINDOW_HEIGHT: i32 = 480;
/// Default maximum number of search results to display
pub const DEFAULT_MAX_RESULTS: usize = 64;
/// Default calculator feature state (disabled by default)
pub const DEFAULT_CALCULATOR: bool = false;
/// Default debounce time in milliseconds for command execution
pub const DEFAULT_COMMAND_DEBOUNCE_MS: u32 = 300;

/// Get the default list of application directories to scan
///
/// These directories contain `.desktop` files that Grunner indexes
/// to populate the application launcher. The list includes:
/// - System-wide application directories
/// - User-local application directories
/// - Flatpak application directories (both system and user)
pub fn default_app_dirs() -> Vec<String> {
    vec![
        "/usr/share/applications".into(),
        "/usr/local/share/applications".into(),
        "~/.local/share/applications".into(),
        "/var/lib/flatpak/exports/share/applications".into(),
        "~/.local/share/flatpak/exports/share/applications".into(),
    ]
}

/// Obsidian-specific configuration
///
/// This struct holds all settings related to Obsidian integration,
/// including vault location and note folder paths.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ObsidianConfig {
    /// Path to the Obsidian vault (supports ~ for home directory)
    pub vault: String,
    /// Folder name for daily notes within the vault
    pub daily_notes_folder: String,
    /// Folder name for newly created notes
    pub new_notes_folder: String,
    /// Filename for the quick note file
    pub quick_note: String,
}

/// Main configuration structure for Grunner
///
/// This struct holds all configurable application settings.
/// It provides sensible defaults for all fields and can be
/// customized via the TOML configuration file.
#[derive(Debug, Clone)]
pub struct Config {
    /// Window width in pixels
    pub window_width: i32,
    /// Window height in pixels
    pub window_height: i32,
    /// Maximum number of search results to display
    pub max_results: usize,
    /// Directories to scan for .desktop files (expanded paths)
    pub app_dirs: Vec<PathBuf>,
    /// Whether the calculator feature is enabled
    pub calculator: bool,
    /// Custom shell commands for search modes (key = mode, value = command)
    pub commands: HashMap<String, String>,
    /// Optional Obsidian integration configuration
    pub obsidian: Option<ObsidianConfig>,
    /// Debounce time in milliseconds for command execution
    pub command_debounce_ms: u32,
    /// List of search provider IDs to exclude from results
    pub search_provider_blacklist: Vec<String>,
}

impl Default for Config {
    /// Create a default configuration with sensible values
    ///
    /// The default configuration includes:
    /// - Standard window dimensions
    /// - Default search result limit
    /// - Common application directories
    /// - Built-in file search commands
    /// - Disabled calculator
    /// - Empty Obsidian configuration
    fn default() -> Self {
        // Initialize default shell commands for file search modes
        let mut commands = HashMap::new();
        commands.insert(
            "f".to_string(),
            "plocate -i -- \"$1\" 2>/dev/null | grep \"^$HOME/\" | head -20".to_string(),
        );
        commands.insert(
            "fg".to_string(),
            "rg --with-filename --line-number --no-heading -S \"$1\" ~ 2>/dev/null | head -20"
                .to_string(),
        );

        Self {
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            max_results: DEFAULT_MAX_RESULTS,
            // Expand ~ in directory paths to actual home directory
            app_dirs: default_app_dirs()
                .into_iter()
                .map(|s| expand_home(&s))
                .collect(),
            calculator: DEFAULT_CALCULATOR,
            commands,
            obsidian: None,
            command_debounce_ms: DEFAULT_COMMAND_DEBOUNCE_MS,
            search_provider_blacklist: Vec::new(),
        }
    }
}

/// Internal TOML configuration structure for deserialization
///
/// This struct mirrors the structure of the TOML configuration file.
/// It uses Option types for all fields to support partial configuration.
#[derive(Deserialize, Serialize, Default)]
struct TomlConfig {
    /// Window-related settings
    window: Option<WindowConfig>,
    /// Search-related settings
    search: Option<SearchConfig>,
    /// Calculator feature settings
    calculator: Option<CalculatorConfig>,
    /// Custom command definitions
    commands: Option<HashMap<String, String>>,
    /// Obsidian integration settings
    obsidian: Option<ObsidianConfig>,
}

/// Window configuration section in TOML
#[derive(Deserialize, Serialize)]
struct WindowConfig {
    /// Optional window width override
    width: Option<i32>,
    /// Optional window height override
    height: Option<i32>,
}

/// Search configuration section in TOML
#[derive(Deserialize, Serialize)]
struct SearchConfig {
    /// Optional maximum results limit
    max_results: Option<usize>,
    /// Optional list of application directories
    app_dirs: Option<Vec<String>>,
    /// Optional command debounce time
    command_debounce_ms: Option<u32>,
    /// Optional search provider blacklist
    provider_blacklist: Option<Vec<String>>,
}

/// Calculator configuration section in TOML
#[derive(Deserialize, Serialize)]
struct CalculatorConfig {
    /// Optional calculator enabled state
    enabled: Option<bool>,
}

/// Get the path to the user's configuration file
///
/// The configuration file is located at:
/// `$HOME/.config/grunner/grunner.toml`
///
/// Returns: `PathBuf` to the configuration file
pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".config")
        .join("grunner")
        .join("grunner.toml")
}

/// Load configuration from file or create default configuration
///
/// This function:
/// 1. Checks if a configuration file exists at the expected path
/// 2. If not, creates the directory and writes a default configuration file
/// 3. Reads and parses the TOML configuration file
/// 4. Merges file settings with defaults (file settings take precedence)
/// 5. Returns the final configuration
///
/// Returns: `Config` struct with loaded or default settings
pub fn load() -> Config {
    let path = config_path();

    // If config file doesn't exist, create it with defaults
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        std::fs::write(&path, default_toml()).ok();
        return Config::default();
    }

    // Read existing config file
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read config file: {}. Using defaults.", e);
            return Config::default();
        }
    };

    // Parse TOML and apply to default configuration
    apply_toml(&content)
}

/// Parse TOML content and apply it to the default configuration
///
/// # Arguments
/// * `content` - TOML configuration string to parse
///
/// # Returns
/// `Config` struct with TOML settings applied on top of defaults
///
/// # Notes
/// - Invalid TOML syntax falls back to defaults with an error message
/// - Individual setting parse errors are ignored (that setting keeps its default)
fn apply_toml(content: &str) -> Config {
    let mut cfg = Config::default();

    // Parse TOML content
    let toml_cfg: TomlConfig = match toml::from_str(content) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to parse config: {}. Using defaults.", e);
            return cfg;
        }
    };

    // Apply window settings if present
    if let Some(window) = toml_cfg.window {
        if let Some(w) = window.width.filter(|&v| v > 0) {
            cfg.window_width = w;
        }
        if let Some(h) = window.height.filter(|&v| v > 0) {
            cfg.window_height = h;
        }
    }

    // Apply search settings if present
    if let Some(search) = toml_cfg.search {
        if let Some(m) = search.max_results.filter(|&v| v > 0) {
            cfg.max_results = m;
        }
        if let Some(dirs) = search.app_dirs {
            cfg.app_dirs = dirs.into_iter().map(|s| expand_home(&s)).collect();
        }
        if let Some(debounce) = search.command_debounce_ms {
            cfg.command_debounce_ms = debounce;
        }
        if let Some(blacklist) = search.provider_blacklist {
            cfg.search_provider_blacklist = blacklist;
        }
    }

    // Apply calculator settings if present
    if let Some(calc) = toml_cfg.calculator {
        if let Some(enabled) = calc.enabled {
            cfg.calculator = enabled;
        }
    }

    // Apply custom commands if present (replaces defaults)
    if let Some(cmds) = toml_cfg.commands {
        cfg.commands = cmds;
    }

    // Apply Obsidian settings if present
    if let Some(obs) = toml_cfg.obsidian {
        cfg.obsidian = Some(obs);
    }

    cfg
}

/// Generate default TOML configuration content
///
/// Creates a well-commented TOML template with all available options
/// and their default values. This is written to disk when no
/// configuration file exists.
///
/// Returns: String containing the default TOML configuration
fn default_toml() -> String {
    let dirs = default_app_dirs()
        .iter()
        .map(|d| format!("    \"{}\",", d))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"# grunner configuration
# All values are optional — missing keys fall back to the built-in defaults.

[window]
# Width and height of the launcher window in pixels.
width  = {width}
height = {height}

[search]
# Maximum number of fuzzy-search results shown (only when a query is active).
max_results = {max}

# Delay in milliseconds before executing a colon command (e.g. :f, :ob) after you stop typing.
# Lower values feel more responsive but may cause flickering if your command is very fast.
command_debounce_ms = 300

# Directories scanned for .desktop files.
# Use ~ for the home directory. Directories that do not exist are skipped.
app_dirs = [
{dirs}
]

# List of GNOME Shell search providers to exclude.
# Use the DesktopId as it appears in the provider's .ini file.
# provider_blacklist = [
#     "org.gnome.Software.desktop",
#     "org.gnome.Characters.desktop",
# ]

[calculator]
# Enable inline calculator (evaluates expressions typed in the search bar).
enabled = false

[commands]
# Define colon commands. The key is the command name (without the leading ':').
# The value is a shell command that will be executed with 'sh -c'.
# Use "$1" for the argument typed after the command.
# f  = "plocate -i -- \"$1\" 2>/dev/null | grep \"^$HOME/\" | head -20"
# fg = "rg --with-filename --line-number --no-heading -S \"$1\" ~ 2>/dev/null | head -20"

# [obsidian]
# Uncomment and fill in to enable Obsidian integration.
# vault = "~/Documents/Obsidian/MyVault"
# daily_notes_folder = "Daily"
# new_notes_folder = "Inbox"
# quick_note = "Quick.md"
"#,
        width = DEFAULT_WINDOW_WIDTH,
        height = DEFAULT_WINDOW_HEIGHT,
        max = DEFAULT_MAX_RESULTS,
        dirs = dirs,
    )
}
