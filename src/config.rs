use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// Defaults
pub const DEFAULT_WINDOW_WIDTH: i32 = 640;
pub const DEFAULT_WINDOW_HEIGHT: i32 = 480;
pub const DEFAULT_MAX_RESULTS: usize = 64;
pub const DEFAULT_CALCULATOR: bool = true; // <-- NUOVO

pub fn default_app_dirs() -> Vec<String> {
    vec![
        "/usr/share/applications".into(),
        "/usr/local/share/applications".into(),
        "~/.local/share/applications".into(),
        "/var/lib/flatpak/exports/share/applications".into(),
        "~/.local/share/flatpak/exports/share/applications".into(),
    ]
}

// Config struct (public)
#[derive(Debug, Clone)]
pub struct Config {
    pub window_width: i32,
    pub window_height: i32,
    pub max_results: usize,
    pub app_dirs: Vec<PathBuf>,
    pub calculator: bool, // <-- NUOVO
    pub commands: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut commands = HashMap::new();
        commands.insert(
            "f".to_string(),
            "find ~ -name \"$1\" 2>/dev/null | head -20".to_string(),
        );
        commands.insert(
            "fg".to_string(),
            "rg --with-filename --line-number --no-heading \"$1\" ~ 2>/dev/null | head -20"
                .to_string(),
        );
        Self {
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            max_results: DEFAULT_MAX_RESULTS,
            app_dirs: default_app_dirs()
                .into_iter()
                .map(|s| expand_home(&s, &home))
                .collect(),
            calculator: DEFAULT_CALCULATOR, // <-- NUOVO
            commands,
        }
    }
}

// TOML structure
#[derive(Deserialize, Serialize, Default)]
struct TomlConfig {
    window: Option<WindowConfig>,
    search: Option<SearchConfig>,
    calculator: Option<CalculatorConfig>, // <-- NUOVO
    commands: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Serialize)]
struct WindowConfig {
    width: Option<i32>,
    height: Option<i32>,
}

#[derive(Deserialize, Serialize)]
struct SearchConfig {
    max_results: Option<usize>,
    app_dirs: Option<Vec<String>>,
}

// <-- NUOVO
#[derive(Deserialize, Serialize)]
struct CalculatorConfig {
    enabled: Option<bool>,
}

// Config file path
pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".config")
        .join("grunner")
        .join("grunner.toml")
}

// Load config
pub fn load() -> Config {
    let path = config_path();

    // Create default file if missing
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        std::fs::write(&path, default_toml()).ok();
        return Config::default();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read config file: {}. Using defaults.", e);
            return Config::default();
        }
    };

    apply_toml(&content)
}

// Apply TOML values to a Config (with fallback to defaults)
fn apply_toml(content: &str) -> Config {
    let mut cfg = Config::default();

    let toml_cfg: TomlConfig = match toml::from_str(content) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to parse config: {}. Using defaults.", e);
            return cfg;
        }
    };

    if let Some(window) = toml_cfg.window {
        if let Some(w) = window.width.filter(|&v| v > 0) {
            cfg.window_width = w;
        }
        if let Some(h) = window.height.filter(|&v| v > 0) {
            cfg.window_height = h;
        }
    }

    if let Some(search) = toml_cfg.search {
        if let Some(m) = search.max_results.filter(|&v| v > 0) {
            cfg.max_results = m;
        }
        if let Some(dirs) = search.app_dirs {
            let home = std::env::var("HOME").unwrap_or_default();
            cfg.app_dirs = dirs.into_iter().map(|s| expand_home(&s, &home)).collect();
        }
    }

    // <-- NUOVO: Leggi la sezione calculator
    if let Some(calc) = toml_cfg.calculator {
        if let Some(enabled) = calc.enabled {
            cfg.calculator = enabled;
        }
    }

    if let Some(cmds) = toml_cfg.commands {
        cfg.commands = cmds;
    }

    cfg
}

// Expand leading `~` to home directory
fn expand_home(path: &str, home: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        PathBuf::from(home).join(rest)
    } else if path == "~" {
        PathBuf::from(home)
    } else {
        PathBuf::from(path)
    }
}

// Default TOML content (with comments)
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

# Directories scanned for .desktop files.
# Use ~ for the home directory. Directories that do not exist are skipped.
app_dirs = [
{dirs}
]

[calculator]
# Enable inline calculator (evaluates expressions typed in the search bar).
enabled = true

[commands]
# Define colon commands. The key is the command name (without the leading ':').
# The value is a shell command that will be executed with 'sh -c'.
# Use "$1" for the argument typed after the command.
# f  = "find ~ -name \"$1\" 2>/dev/null | head -20"
# fg = "rg --with-filename --line-number --no-heading \"$1\" ~ 2>/dev/null | head -20"
"#,
        width = DEFAULT_WINDOW_WIDTH,
        height = DEFAULT_WINDOW_HEIGHT,
        max = DEFAULT_MAX_RESULTS,
        dirs = dirs,
    )
}
