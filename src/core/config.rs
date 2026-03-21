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

use crate::core::global_state::get_home_dir;
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
#[must_use]
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

/// Custom script command configuration
///
/// This struct holds a saved command with a name, the command to execute,
/// optional working directory, and whether to keep the terminal open.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct CommandConfig {
    /// Name displayed in the launcher (e.g., "Update System")
    pub name: String,
    /// Command to execute (e.g., "sudo apt update")
    pub command: String,
    /// Working directory (empty = home directory)
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Whether to keep the terminal open after executing the command
    #[serde(default = "default_keep_open")]
    pub keep_open: bool,
}

/// Theme mode selection
///
/// Controls the application's color theme. Can follow system preferences
/// or use a specific built-in or custom theme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    /// Follow system light/dark preference
    #[default]
    System,
    /// Force light theme regardless of system
    SystemLight,
    /// Force dark theme regardless of system
    SystemDark,
    /// Tokyo Night theme
    TokioNight,
    /// Catppuccin Mocha (dark)
    CatppuccinMocha,
    /// Catppuccin Latte (light)
    CatppuccinLatte,
    /// Nord theme
    Nord,
    /// Gruvbox Dark
    GruvboxDark,
    /// Gruvbox Light
    GruvboxLight,
    /// Dracula theme
    Dracula,
    /// Custom theme from file
    Custom,
}

fn default_keep_open() -> bool {
    true
}

/// Main configuration structure for Grunner
///
/// This struct holds all configurable application settings.
/// It provides sensible defaults for all fields and can be
/// customized via the TOML configuration file.
#[derive(Debug, Clone, Serialize)]
pub struct Config {
    /// Window width in pixels
    pub window_width: i32,
    /// Window height in pixels
    pub window_height: i32,
    /// Maximum number of search results to display
    pub max_results: usize,
    /// Directories to scan for .desktop files (raw paths, use `expanded_app_dirs()`)
    pub app_dirs: Vec<String>,
    /// Optional Obsidian integration configuration
    pub obsidian: Option<ObsidianConfig>,
    /// Debounce time in milliseconds for command execution
    pub command_debounce_ms: u32,
    /// List of search provider IDs to exclude from results
    pub search_provider_blacklist: Vec<String>,
    /// Whether the workspace window bar is enabled (default: true)
    pub workspace_bar_enabled: bool,
    /// List of custom script commands for :sh mode
    pub commands: Vec<CommandConfig>,
    /// Disable all special modes (colon commands) and hide power bar
    /// Activated via --simple / -s command-line flag or `GRUNNER_SIMPLE` env var
    pub disable_modes: bool,
    /// Theme mode selection
    pub theme: ThemeMode,
    /// Path to custom theme file (used when theme = Custom)
    pub custom_theme_path: Option<String>,
    /// List of pinned (favorite) application desktop entry IDs
    pub pinned_apps: Vec<String>,
}

impl Config {
    /// Get application directories with home paths expanded
    ///
    /// This lazily expands ~ to the actual home directory path.
    /// Call this method when you need actual filesystem paths.
    #[must_use]
    pub fn expanded_app_dirs(&self) -> Vec<PathBuf> {
        self.app_dirs.iter().map(|s| expand_home(s)).collect()
    }
}

impl Default for Config {
    /// Create a default configuration with sensible values
    ///
    /// The default configuration includes:
    /// - Standard window dimensions
    /// - Default search result limit
    /// - Common application directories (stored as raw strings, not expanded)
    /// - Empty commands list (colon commands are built-in mode triggers)
    /// - Obsidian configuration with empty defaults (always present so the UI is visible)
    fn default() -> Self {
        Self {
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            max_results: DEFAULT_MAX_RESULTS,
            app_dirs: default_app_dirs(),
            obsidian: Some(ObsidianConfig::default()),
            command_debounce_ms: DEFAULT_COMMAND_DEBOUNCE_MS,
            search_provider_blacklist: Vec::new(),
            workspace_bar_enabled: true,
            commands: Vec::new(),
            disable_modes: false,
            theme: ThemeMode::default(),
            custom_theme_path: None,
            pinned_apps: Vec::new(),
        }
    }
}

// ── Per-section structs used during TOML parsing ──────────────────────────

#[derive(Deserialize)]
struct WindowConfig {
    width: Option<i32>,
    height: Option<i32>,
}

#[derive(Deserialize)]
struct SearchConfig {
    max_results: Option<usize>,
    app_dirs: Option<Vec<String>>,
    command_debounce_ms: Option<u32>,
    provider_blacklist: Option<Vec<String>>,
    workspace_bar_enabled: Option<bool>,
    pinned_apps: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ThemeConfig {
    mode: Option<ThemeMode>,
    custom_theme_path: Option<String>,
}

/// Get the path to the user's configuration file
///
/// The configuration file is located at:
/// `$HOME/.config/grunner/grunner.toml`
///
/// Returns: `PathBuf` to the configuration file
#[must_use]
pub fn config_path() -> PathBuf {
    let home = get_home_dir();
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
#[must_use]
pub fn load() -> Config {
    let path = config_path();

    // If config file doesn't exist, create it with defaults
    if !path.exists() {
        info!(
            "Configuration file not found at {}, creating default",
            path.display()
        );
        if let Some(dir) = path.parent()
            && std::fs::create_dir_all(dir).is_ok()
        {
            debug!("Created configuration directory: {}", dir.display());
        }
        if std::fs::write(&path, default_toml()).is_ok() {
            info!("Created default configuration file at {}", path.display());
        } else {
            warn!(
                "Failed to create default configuration file at {}",
                path.display()
            );
        }
        return Config::default();
    }

    // Read existing config file
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => {
            debug!(
                "Successfully read configuration file from {}",
                path.display()
            );
            s
        }
        Err(e) => {
            // Failed to read config file
            error!(
                "Failed to read configuration file from {}: {e}",
                path.display()
            );
            return Config::default();
        }
    };

    // Parse TOML and apply to default configuration
    debug!("Parsing configuration TOML ({} bytes)", content.len());
    let (cfg, failed, table) = apply_toml(&content);

    // If sections were malformed, patch only those sections with defaults
    if !failed.is_empty() {
        warn!(
            "Config sections failed to parse ({}), falling back to defaults: {}",
            path.display(),
            failed.join(", ")
        );
        let corrected = patch_failed_sections(table, &failed);
        if std::fs::write(&path, &corrected).is_ok() {
            info!(
                "Patched config file replacing sections [{}] with defaults at {}",
                failed.join(", "),
                path.display()
            );
        }
    }

    cfg
}

/// Parse TOML content and apply it to the default configuration
///
/// Each top-level section is deserialized independently so that a malformed
/// section (e.g. legacy `commands = []` instead of `[[commands]]`) does not
/// prevent the rest of the config from loading.
///
/// # Returns
/// A tuple of `(Config, Vec<String>, toml::value::Table)` where the second
/// element lists section names that failed to parse, and the third is the
/// original parsed table (useful for patching).
fn apply_toml(content: &str) -> (Config, Vec<String>, toml::value::Table) {
    let mut cfg = Config::default();
    let mut failed: Vec<String> = Vec::new();

    let full: toml::Value = match toml::from_str(content) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to parse TOML syntax: {e}");
            return (cfg, failed, toml::value::Table::new());
        }
    };

    let toml::Value::Table(table) = full else {
        return (cfg, failed, toml::value::Table::new());
    };

    // [window]
    if let Some(val) = table.get("window") {
        match parse_section::<WindowConfig>(val) {
            Some(window) => {
                if let Some(w) = window.width.filter(|&v| v > 0) {
                    debug!("Setting window width to {w}");
                    cfg.window_width = w;
                }
                if let Some(h) = window.height.filter(|&v| v > 0) {
                    debug!("Setting window height to {h}");
                    cfg.window_height = h;
                }
            }
            None => failed.push("window".to_string()),
        }
    }

    // [search]
    if let Some(val) = table.get("search") {
        match parse_section::<SearchConfig>(val) {
            Some(search) => {
                if let Some(m) = search.max_results.filter(|&v| v > 0) {
                    debug!("Setting max_results to {m}");
                    cfg.max_results = m;
                }
                if let Some(dirs) = search.app_dirs {
                    debug!("Setting app_dirs to {dirs:?}");
                    cfg.app_dirs = dirs;
                }
                if let Some(debounce) = search.command_debounce_ms {
                    debug!("Setting command_debounce_ms to {debounce}");
                    cfg.command_debounce_ms = debounce;
                }
                if let Some(blacklist) = search.provider_blacklist {
                    debug!("Setting search_provider_blacklist to {blacklist:?}");
                    cfg.search_provider_blacklist = blacklist;
                }
                if let Some(enabled) = search.workspace_bar_enabled {
                    debug!("Setting workspace_bar_enabled to {enabled}");
                    cfg.workspace_bar_enabled = enabled;
                }
                if let Some(pinned) = search.pinned_apps {
                    debug!("Setting pinned_apps to {pinned:?}");
                    cfg.pinned_apps = pinned;
                }
            }
            None => failed.push("search".to_string()),
        }
    }

    // [obsidian]
    if let Some(val) = table.get("obsidian") {
        match parse_section::<ObsidianConfig>(val) {
            Some(obs) => {
                debug!("Setting Obsidian configuration");
                cfg.obsidian = Some(obs);
            }
            None => failed.push("obsidian".to_string()),
        }
    }

    // [[commands]]
    if let Some(val) = table.get("commands") {
        match parse_section::<Vec<CommandConfig>>(val) {
            Some(cmds) => {
                debug!("Setting custom script commands: {} commands", cmds.len());
                cfg.commands = cmds;
            }
            None => failed.push("commands".to_string()),
        }
    }

    // [theme]
    if let Some(val) = table.get("theme") {
        match parse_section::<ThemeConfig>(val) {
            Some(theme) => {
                if let Some(mode) = theme.mode {
                    debug!("Setting theme mode to {mode:?}");
                    cfg.theme = mode;
                }
                if let Some(path) = theme.custom_theme_path {
                    debug!("Setting custom theme path to {path}");
                    cfg.custom_theme_path = Some(path);
                }
            }
            None => failed.push("theme".to_string()),
        }
    }

    (cfg, failed, table)
}

/// Try to deserialize a `toml::Value` into `T`, logging a warning on failure.
fn parse_section<T: serde::de::DeserializeOwned>(val: &toml::Value) -> Option<T> {
    match val.clone().try_into::<T>() {
        Ok(v) => Some(v),
        Err(e) => {
            warn!("Failed to parse config section: {e}");
            None
        }
    }
}

/// Replace only the given failed sections in the original TOML table with
/// their defaults, then re-serialize.  Sections that parsed correctly are
/// left untouched so their values, ordering, and any surrounding content
/// are preserved.
fn patch_failed_sections(mut table: toml::value::Table, failed: &[String]) -> String {
    // Build default Config → serialize → parse back to a Value table
    let default_val = Config::default();
    let default_toml = config_to_toml(&default_val);
    let default_root: toml::Value = match toml::from_str(&default_toml) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to parse default config for patching: {e}");
            return toml::to_string_pretty(&toml::Value::Table(table)).unwrap_or_default();
        }
    };
    let toml::Value::Table(default_table) = default_root else {
        return toml::to_string_pretty(&toml::Value::Table(table)).unwrap_or_default();
    };

    for section in failed {
        if let Some(default_section) = default_table.get(section) {
            table.insert(section.clone(), default_section.clone());
        }
    }

    toml::to_string_pretty(&toml::Value::Table(table)).unwrap_or_default()
}

/// Serialize a `Config` back to TOML, matching the file layout.
///
/// `None` fields are omitted so the file stays clean.
///
/// # Panics
///
/// Panics if TOML serialization of a valid `Config` fails (should never happen).
#[must_use]
pub fn config_to_toml(config: &Config) -> String {
    #[derive(Serialize)]
    struct TomlConfig<'a> {
        window: WindowConfig,
        search: SearchConfig<'a>,
        obsidian: Option<&'a ObsidianConfig>,
        commands: &'a [CommandConfig],
        theme: ThemeConfig,
    }
    #[derive(Serialize)]
    struct WindowConfig {
        width: i32,
        height: i32,
    }
    #[derive(Serialize)]
    struct SearchConfig<'a> {
        max_results: usize,
        app_dirs: &'a [String],
        command_debounce_ms: u32,
        provider_blacklist: &'a [String],
        workspace_bar_enabled: bool,
        pinned_apps: &'a [String],
    }
    #[derive(Serialize)]
    struct ThemeConfig {
        mode: ThemeMode,
        custom_theme_path: Option<String>,
    }

    let tc = TomlConfig {
        window: WindowConfig {
            width: config.window_width,
            height: config.window_height,
        },
        search: SearchConfig {
            max_results: config.max_results,
            app_dirs: &config.app_dirs,
            command_debounce_ms: config.command_debounce_ms,
            provider_blacklist: &config.search_provider_blacklist,
            workspace_bar_enabled: config.workspace_bar_enabled,
            pinned_apps: &config.pinned_apps,
        },
        obsidian: config.obsidian.as_ref(),
        commands: &config.commands,
        theme: ThemeConfig {
            mode: config.theme,
            custom_theme_path: config.custom_theme_path.clone(),
        },
    };

    toml::to_string_pretty(&tc).expect("config serialization should never fail")
}

/// Generate default TOML configuration content
///
/// Creates a well-commented TOML template with all available options
/// and their default values. This is written to disk when no
/// configuration file exists.
///
/// Returns: String containing the default TOML configuration
#[allow(clippy::uninlined_format_args)]
fn default_toml() -> String {
    let dirs = default_app_dirs()
        .iter()
        .map(|d| format!("    \"{d}\","))
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
command_debounce_ms = {debounce}

# Directories scanned for .desktop files.
# Use ~ for the home directory. Directories that do not exist are skipped.
app_dirs = [
{dirs}
]

# List of GNOME Shell search providers to exclude.
# Use the DesktopId as it appears in the provider's .ini file.
provider_blacklist = []

# Enable workspace window bar (requires window-calls GNOME Shell extension).
# Install from: https://extensions.gnome.org/extension/4724/window-calls/
workspace_bar_enabled = true

# List of pinned (favorite) application desktop entry IDs.
# These appear as quick-access icons above the search results.
# Example: pinned_apps = ["firefox.desktop", "org.gnome.Terminal.desktop"]
pinned_apps = []

[obsidian]
vault = ""
daily_notes_folder = ""
new_notes_folder = ""
quick_note = ""

# Custom script commands for :sh mode
# These commands will appear when you type :sh in the launcher
# Example:
# [[commands]]
# name = "Update System"
# command = "sudo apt update"
#
# [[commands]]
# name = "Update Flatpaks"
# command = "flatpak update"

[theme]
# Theme mode selection
# Options: system, system-light, system-dark, tokio-night, catppuccin-mocha, 
#          catppuccin-latte, nord, gruvbox-dark, gruvbox-light, dracula, custom
mode = "system"

# Path to custom theme CSS file (only used when mode = "custom")
# Example: custom_theme_path = "~/.config/grunner/themes/my_theme.css"
"#,
        width = DEFAULT_WINDOW_WIDTH,
        height = DEFAULT_WINDOW_HEIGHT,
        max = DEFAULT_MAX_RESULTS,
        debounce = DEFAULT_COMMAND_DEBOUNCE_MS,
        dirs = dirs,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_app_dirs() {
        let dirs = default_app_dirs();
        assert_eq!(dirs.len(), 5);
        assert!(dirs[0].contains("/usr/share/applications"));
        assert!(dirs[1].contains("/usr/local/share/applications"));
        assert!(dirs[2].contains("~/.local/share/applications"));
        assert!(dirs[3].contains("/var/lib/flatpak/exports/share/applications"));
        assert!(dirs[4].contains("~/.local/share/flatpak/exports/share/applications"));
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.window_width, DEFAULT_WINDOW_WIDTH);
        assert_eq!(config.window_height, DEFAULT_WINDOW_HEIGHT);
        assert_eq!(config.max_results, DEFAULT_MAX_RESULTS);
        assert_eq!(config.command_debounce_ms, DEFAULT_COMMAND_DEBOUNCE_MS);
        assert!(config.app_dirs.len() > 0);
        assert!(config.workspace_bar_enabled);
        assert!(config.obsidian.is_some());
        assert!(config.pinned_apps.is_empty());
    }

    #[test]
    fn test_expanded_app_dirs() {
        let config = Config {
            app_dirs: vec!["~/.local/share/applications".to_string()],
            ..Default::default()
        };
        let expanded = config.expanded_app_dirs();
        assert_eq!(expanded.len(), 1);
        assert!(expanded[0].to_string_lossy().starts_with("/home"));
    }

    #[test]
    fn test_default_toml_generation() {
        let toml = default_toml();
        assert!(toml.contains("workspace_bar_enabled = true"));
        assert!(toml.contains("max_results"));
        assert!(toml.contains("command_debounce_ms"));
    }

    #[test]
    fn test_apply_toml_workspace_bar_enabled() {
        // Test enabling workspace bar
        let toml = r#"
            [search]
            workspace_bar_enabled = true
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert!(config.workspace_bar_enabled);
        assert!(failed.is_empty());

        // Test disabling workspace bar
        let toml = r#"
            [search]
            workspace_bar_enabled = false
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert!(!config.workspace_bar_enabled);
        assert!(failed.is_empty());
    }

    #[test]
    fn test_apply_toml_window_settings() {
        let toml = r#"
            [window]
            width = 800
            height = 600
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert_eq!(config.window_width, 800);
        assert_eq!(config.window_height, 600);
        assert!(failed.is_empty());
    }

    #[test]
    fn test_apply_toml_search_settings() {
        let toml = r#"
            [search]
            max_results = 100
            command_debounce_ms = 500
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert_eq!(config.max_results, 100);
        assert_eq!(config.command_debounce_ms, 500);
        assert!(failed.is_empty());
    }

    #[test]
    fn test_apply_toml_invalid_values() {
        // Negative width should be ignored
        let toml = r#"
            [window]
            width = -100
        "#;
        let (config, _failed, _table) = apply_toml(toml);
        assert_eq!(config.window_width, DEFAULT_WINDOW_WIDTH);

        // Zero max_results should be ignored
        let toml = r#"
            [search]
            max_results = 0
        "#;
        let (config, _failed, _table) = apply_toml(toml);
        assert_eq!(config.max_results, DEFAULT_MAX_RESULTS);
    }

    #[test]
    fn test_obsidian_config() {
        let obsidian = ObsidianConfig {
            vault: "~/obsidian".to_string(),
            daily_notes_folder: "daily".to_string(),
            new_notes_folder: "new".to_string(),
            quick_note: "quick.md".to_string(),
        };
        assert_eq!(obsidian.vault, "~/obsidian");
        assert_eq!(obsidian.daily_notes_folder, "daily");
    }

    #[test]
    fn test_apply_toml_missing_commands() {
        // Test that missing commands field defaults to empty Vec
        let toml = r#"
            [window]
            width = 800
            height = 600
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert!(config.commands.is_empty());
        assert_eq!(config.commands.len(), 0);
        assert!(failed.is_empty());
    }

    #[test]
    fn test_apply_toml_with_commands() {
        // Test that commands field is correctly parsed
        let toml = r#"
            [[commands]]
            name = "Test Command"
            command = "echo test"
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert_eq!(config.commands.len(), 1);
        assert_eq!(config.commands[0].name, "Test Command");
        assert_eq!(config.commands[0].command, "echo test");
        assert!(failed.is_empty());
    }

    #[test]
    fn test_config_default_has_empty_commands() {
        // Test that default config has empty commands Vec
        let config = Config::default();
        assert!(config.commands.is_empty());
    }

    #[test]
    fn test_apply_toml_pinned_apps() {
        let toml = r#"
            [search]
            pinned_apps = ["firefox.desktop", "org.gnome.Terminal.desktop"]
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert_eq!(config.pinned_apps.len(), 2);
        assert_eq!(config.pinned_apps[0], "firefox.desktop");
        assert_eq!(config.pinned_apps[1], "org.gnome.Terminal.desktop");
        assert!(failed.is_empty());
    }

    #[test]
    fn test_apply_toml_legacy_commands_incompatible() {
        // Old format: `commands = [1, 2, 3]` at top level — values are not tables.
        // The section fails to parse, but the rest of the config is fine.
        let toml = r#"
            commands = [1, 2, 3]

            [window]
            width = 800
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert_eq!(config.window_width, 800);
        assert!(config.commands.is_empty());
        assert!(failed.contains(&"commands".to_string()));
    }

    #[test]
    fn test_apply_toml_wrong_type_in_section() {
        // width is a string instead of int
        let toml = r#"
            [window]
            width = "not a number"
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert_eq!(config.window_width, DEFAULT_WINDOW_WIDTH);
        assert!(failed.contains(&"window".to_string()));

        // valid sections alongside invalid ones still load
        let toml = r#"
            [window]
            width = "not a number"

            [search]
            max_results = 50
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert_eq!(config.window_width, DEFAULT_WINDOW_WIDTH);
        assert_eq!(config.max_results, 50);
        assert_eq!(config.command_debounce_ms, DEFAULT_COMMAND_DEBOUNCE_MS);
        assert!(failed.contains(&"window".to_string()));
        assert!(!failed.contains(&"search".to_string()));
    }

    #[test]
    fn test_apply_toml_multiple_invalid_sections() {
        let toml = r#"
            [window]
            width = "bad"

            [obsidian]
            vault = 42

            [[commands]]
            name = "Good"
            command = "echo ok"
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert_eq!(config.window_width, DEFAULT_WINDOW_WIDTH);
        assert!(config.obsidian.is_some()); // default, not the invalid one
        assert_eq!(config.commands.len(), 1);
        assert_eq!(config.commands[0].name, "Good");
        assert!(failed.contains(&"window".to_string()));
        assert!(failed.contains(&"obsidian".to_string()));
        assert!(!failed.contains(&"commands".to_string()));
    }

    #[test]
    fn test_apply_toml_auto_corrects_and_saves() {
        use std::fs;

        let dir = std::env::temp_dir().join("grunner_test_autosave");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("grunner.toml");

        // Write a config with a malformed commands section (values, not tables)
        let bad_toml = r#"
            commands = [1, 2, 3]

            [window]
            width = 900
            height = 500

            [search]
            max_results = 25
        "#;
        fs::write(&path, bad_toml).unwrap();

        // Simulate what load() does
        let content = fs::read_to_string(&path).unwrap();
        let (cfg, failed, table) = apply_toml(&content);

        assert_eq!(cfg.window_width, 900);
        assert_eq!(cfg.max_results, 25);
        assert!(cfg.commands.is_empty());
        assert!(failed.contains(&"commands".to_string()));

        // Patch only the failed section
        let corrected = patch_failed_sections(table, &failed);
        fs::write(&path, &corrected).unwrap();

        // Re-read and verify it parses cleanly now
        let content2 = fs::read_to_string(&path).unwrap();
        let (cfg2, failed2, _table) = apply_toml(&content2);
        assert!(failed2.is_empty());
        assert_eq!(cfg2.window_width, 900);
        assert_eq!(cfg2.max_results, 25);
        assert_eq!(cfg2.window_height, 500);

        // Verify the corrected file re-parses cleanly
        assert!(!corrected.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_to_toml_round_trip() {
        let mut config = Config::default();
        config.window_width = 1024;
        config.window_height = 768;
        config.max_results = 128;
        config.command_debounce_ms = 500;
        config.workspace_bar_enabled = false;
        config.pinned_apps = vec!["firefox.desktop".into()];

        let toml_str = config_to_toml(&config);
        let (parsed, failed, _table) = apply_toml(&toml_str);

        assert!(failed.is_empty());
        assert_eq!(parsed.window_width, 1024);
        assert_eq!(parsed.window_height, 768);
        assert_eq!(parsed.max_results, 128);
        assert_eq!(parsed.command_debounce_ms, 500);
        assert!(!parsed.workspace_bar_enabled);
        assert_eq!(parsed.pinned_apps, vec!["firefox.desktop"]);
    }

    #[test]
    fn test_apply_toml_empty_string() {
        let (config, failed, _table) = apply_toml("");
        // Should return defaults, no failures
        assert!(failed.is_empty());
        assert_eq!(config.window_width, DEFAULT_WINDOW_WIDTH);
        assert_eq!(config.window_height, DEFAULT_WINDOW_HEIGHT);
    }

    #[test]
    fn test_apply_toml_theme_settings() {
        let toml = r#"
            [theme]
            mode = "system-dark"
            custom_theme_path = "~/my_theme.css"
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert!(failed.is_empty());
        assert_eq!(config.theme, ThemeMode::SystemDark);
        assert_eq!(config.custom_theme_path, Some("~/my_theme.css".to_string()));
    }

    #[test]
    fn test_apply_toml_provider_blacklist() {
        let toml = r#"
            [search]
            provider_blacklist = ["org.gnome.Calculator", "org.gnome.Calendar"]
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert!(failed.is_empty());
        assert_eq!(config.search_provider_blacklist.len(), 2);
        assert_eq!(config.search_provider_blacklist[0], "org.gnome.Calculator");
    }

    #[test]
    fn test_apply_toml_obsidian_config() {
        let toml = r#"
            [obsidian]
            vault = "~/vault"
            daily_notes_folder = "Daily"
            new_notes_folder = "Inbox"
            quick_note = "Quick.md"
        "#;
        let (config, failed, _table) = apply_toml(toml);
        assert!(failed.is_empty());
        let obs = config.obsidian.unwrap();
        assert_eq!(obs.vault, "~/vault");
        assert_eq!(obs.daily_notes_folder, "Daily");
        assert_eq!(obs.new_notes_folder, "Inbox");
        assert_eq!(obs.quick_note, "Quick.md");
    }

    #[test]
    fn test_patch_failed_sections_preserves_valid() {
        let toml = r#"
            [window]
            width = "bad"

            [search]
            max_results = 42
        "#;
        let (_cfg, failed, table) = apply_toml(toml);
        assert!(failed.contains(&"window".to_string()));

        let patched = patch_failed_sections(table, &failed);
        let (re_parsed, re_failed, _table) = apply_toml(&patched);
        assert!(re_failed.is_empty());
        assert_eq!(re_parsed.window_width, DEFAULT_WINDOW_WIDTH);
        assert_eq!(re_parsed.max_results, 42);
    }
}
