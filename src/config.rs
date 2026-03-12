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
//! - Obsidian vault integration settings
//! - Search provider filtering

use crate::utils::expand_home;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Default window width in pixels
pub const DEFAULT_WINDOW_WIDTH: i32 = 640;
/// Default window height in pixels
pub const DEFAULT_WINDOW_HEIGHT: i32 = 480;
/// Default maximum number of search results to display
pub const DEFAULT_MAX_RESULTS: usize = 64;
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
    /// Optional Obsidian integration configuration
    pub obsidian: Option<ObsidianConfig>,
    /// Debounce time in milliseconds for command execution
    pub command_debounce_ms: u32,
    /// List of search provider IDs to exclude from results
    pub search_provider_blacklist: Vec<String>,
    /// Whether the workspace window bar is enabled (default: true)
    pub workspace_bar_enabled: bool,
}

impl Default for Config {
    /// Create a default configuration with sensible values
    ///
    /// The default configuration includes:
    /// - Standard window dimensions
    /// - Default search result limit
    /// - Common application directories
    /// - Fixed colon commands (:ob, :obg, :f, :fg)
    /// - Empty Obsidian configuration
    fn default() -> Self {
        Self {
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            max_results: DEFAULT_MAX_RESULTS,
            // Expand ~ in directory paths to actual home directory
            app_dirs: default_app_dirs()
                .into_iter()
                .map(|s| expand_home(&s))
                .collect(),
            obsidian: None,
            command_debounce_ms: DEFAULT_COMMAND_DEBOUNCE_MS,
            search_provider_blacklist: Vec::new(),
            workspace_bar_enabled: true,
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
    /// Optional workspace bar enabled flag (default: true)
    workspace_bar_enabled: Option<bool>,
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
        info!(
            "Configuration file not found at {:?}, creating default",
            path
        );
        if let Some(dir) = path.parent() {
            if std::fs::create_dir_all(dir).is_ok() {
                debug!("Created configuration directory: {:?}", dir);
            }
        }
        if std::fs::write(&path, default_toml()).is_ok() {
            info!("Created default configuration file at {:?}", path);
        } else {
            warn!("Failed to create default configuration file at {:?}", path);
        }
        return Config::default();
    }

    // Read existing config file
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => {
            debug!("Successfully read configuration file from {:?}", path);
            s
        }
        Err(e) => {
            // Failed to read config file
            error!("Failed to read configuration file from {:?}: {}", path, e);
            return Config::default();
        }
    };

    // Parse TOML and apply to default configuration
    debug!("Parsing configuration TOML ({} bytes)", content.len());
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
        Ok(c) => {
            debug!("Successfully parsed configuration TOML");
            c
        }
        Err(e) => {
            // Failed to parse config
            error!("Failed to parse configuration TOML: {}", e);
            return cfg;
        }
    };

    // Apply window settings if present
    if let Some(window) = toml_cfg.window {
        if let Some(w) = window.width.filter(|&v| v > 0) {
            debug!("Setting window width to {}", w);
            cfg.window_width = w;
        }
        if let Some(h) = window.height.filter(|&v| v > 0) {
            debug!("Setting window height to {}", h);
            cfg.window_height = h;
        }
    }

    // Apply search settings if present
    if let Some(search) = toml_cfg.search {
        if let Some(m) = search.max_results.filter(|&v| v > 0) {
            debug!("Setting max_results to {}", m);
            cfg.max_results = m;
        }
        if let Some(dirs) = search.app_dirs {
            debug!("Setting app_dirs to {:?}", dirs);
            cfg.app_dirs = dirs.into_iter().map(|s| expand_home(&s)).collect();
        }
        if let Some(debounce) = search.command_debounce_ms {
            debug!("Setting command_debounce_ms to {}", debounce);
            cfg.command_debounce_ms = debounce;
        }
        if let Some(blacklist) = search.provider_blacklist {
            debug!("Setting search_provider_blacklist to {:?}", blacklist);
            cfg.search_provider_blacklist = blacklist;
        }
        if let Some(enabled) = search.workspace_bar_enabled {
            debug!("Setting workspace_bar_enabled to {}", enabled);
            cfg.workspace_bar_enabled = enabled;
        }
    }

    // Apply Obsidian settings if present
    if let Some(obs) = toml_cfg.obsidian {
        debug!("Setting Obsidian configuration");
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

# Delay in milliseconds before executing a colon command (e.g. :ob, :obg, :f, :fg) after you stop typing.
# Lower values feel more responsive but may cause flickering if your command is very fast.
command_debounce_ms = 300

# Directories scanned for .desktop files.
# Use ~ for the home directory. Directories that do not exist are skipped.
app_dirs = [
{dirs}
]

# List of GNOME Shell search providers to exclude.
# Use the DesktopId as it appears in the provider's .ini file.
provider_blacklist = []

# Enable workspace window bar (requires GNOME Shell with --unsafe-mode or appropriate permissions).
workspace_bar_enabled = true

[obsidian]
vault = ""
daily_notes_folder = ""
new_notes_folder = ""
quick_note = ""
"#,
        width = DEFAULT_WINDOW_WIDTH,
        height = DEFAULT_WINDOW_HEIGHT,
        max = DEFAULT_MAX_RESULTS,
        dirs = dirs,
    )
}
