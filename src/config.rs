use std::path::PathBuf;

// ── Defaults ──────────────────────────────────────────────────────────────────

pub const DEFAULT_WINDOW_WIDTH: i32 = 640;
pub const DEFAULT_WINDOW_HEIGHT: i32 = 480;
pub const DEFAULT_MAX_RESULTS: usize = 64;

pub fn default_app_dirs() -> Vec<String> {
    vec![
        "/usr/share/applications".into(),
        "/usr/local/share/applications".into(),
        "~/.local/share/applications".into(),
        "/var/lib/flatpak/exports/share/applications".into(),
        "~/.local/share/flatpak/exports/share/applications".into(),
    ]
}

// ── Config struct ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Config {
    pub window_width: i32,
    pub window_height: i32,
    pub max_results: usize,
    /// Paths to scan for .desktop files. `~` is expanded to $HOME.
    pub app_dirs: Vec<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        Self {
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            max_results: DEFAULT_MAX_RESULTS,
            app_dirs: default_app_dirs()
                .into_iter()
                .map(|s| expand_home(&s, &home))
                .collect(),
        }
    }
}

// ── Config file path ──────────────────────────────────────────────────────────

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".config")
        .join("grunner")
        .join("grunner.toml")
}

// ── Load ──────────────────────────────────────────────────────────────────────

/// Loads the config from disk, creating the file with defaults if it does not
/// exist. On any parse error the affected field is silently ignored and its
/// default value is kept, so a partially-edited file always works.
pub fn load() -> Config {
    let path = config_path();

    // Create config dir + file with documented defaults if missing.
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        std::fs::write(&path, default_toml()).ok();
        return Config::default();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Config::default(),
    };

    parse_toml(&content)
}

// ── Parser ────────────────────────────────────────────────────────────────────
//
// Hand-rolled parser for the small subset of TOML we need:
//   [section]
//   key = integer
//   key = [ "str", "str", ... ]       (single-line or multi-line array)
//
// Unknown keys and sections are ignored, keeping the file forward-compatible.

fn parse_toml(content: &str) -> Config {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut cfg = Config::default();

    let mut section = String::new();
    // State for multi-line array accumulation.
    let mut array_key: Option<String> = None;
    let mut array_buf = String::new();

    for raw_line in content.lines() {
        let line = strip_comment(raw_line).trim().to_string();

        // ── Multi-line array accumulation ──────────────────────────────────
        if let Some(ref key) = array_key.clone() {
            array_buf.push_str(&line);
            array_buf.push(' ');
            if line.contains(']') {
                let strings = parse_string_array(&array_buf);
                apply_string_array(&mut cfg, &section, key, strings, &home);
                array_key = None;
                array_buf.clear();
            }
            continue;
        }

        // ── Section header ─────────────────────────────────────────────────
        if line.starts_with('[') && line.ends_with(']') && !line.starts_with("[[") {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }

        // ── Key = Value ────────────────────────────────────────────────────
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim();

        if value.starts_with('[') {
            if value.ends_with(']') {
                // Single-line array
                let strings = parse_string_array(value);
                apply_string_array(&mut cfg, &section, &key, strings, &home);
            } else {
                // Multi-line array — start accumulating
                array_key = Some(key);
                array_buf = value.to_string();
                array_buf.push(' ');
            }
        } else {
            apply_scalar(&mut cfg, &section, &key, value);
        }
    }

    cfg
}

fn apply_scalar(cfg: &mut Config, section: &str, key: &str, value: &str) {
    match (section, key) {
        ("window", "width") => {
            if let Ok(v) = value.parse::<i32>() {
                if v > 0 {
                    cfg.window_width = v;
                }
            }
        }
        ("window", "height") => {
            if let Ok(v) = value.parse::<i32>() {
                if v > 0 {
                    cfg.window_height = v;
                }
            }
        }
        ("search", "max_results") => {
            if let Ok(v) = value.parse::<usize>() {
                if v > 0 {
                    cfg.max_results = v;
                }
            }
        }
        _ => {}
    }
}

fn apply_string_array(
    cfg: &mut Config,
    section: &str,
    key: &str,
    strings: Vec<String>,
    home: &str,
) {
    if section == "search" && key == "app_dirs" && !strings.is_empty() {
        cfg.app_dirs = strings
            .into_iter()
            .map(|s| expand_home(&s, home))
            .collect();
    }
}

/// Parses `[ "a", "b", "c" ]` → `vec!["a", "b", "c"]`
fn parse_string_array(s: &str) -> Vec<String> {
    let inner = s
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();

    let mut result = Vec::new();
    let mut in_str = false;
    let mut current = String::new();

    for ch in inner.chars() {
        match ch {
            '"' => {
                if in_str {
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        result.push(trimmed);
                    }
                    current.clear();
                }
                in_str = !in_str;
            }
            _ if in_str => current.push(ch),
            _ => {}
        }
    }

    result
}

/// Strips inline `#` comments (respects quoted strings).
fn strip_comment(line: &str) -> &str {
    let mut in_str = false;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => in_str = !in_str,
            '#' if !in_str => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Expands a leading `~` to `home`.
fn expand_home(path: &str, home: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        PathBuf::from(home).join(rest)
    } else if path == "~" {
        PathBuf::from(home)
    } else {
        PathBuf::from(path)
    }
}

// ── Default TOML template ─────────────────────────────────────────────────────

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
"#,
        width = DEFAULT_WINDOW_WIDTH,
        height = DEFAULT_WINDOW_HEIGHT,
        max = DEFAULT_MAX_RESULTS,
        dirs = dirs,
    )
}