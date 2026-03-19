//! Configuration persistence for the settings window.
//!
//! Kept separate from the UI code so serialisation logic can be
//! read and tested independently of GTK.

use crate::core::config::{self, CommandConfig, Config, ObsidianConfig, ThemeMode};
use log::debug;
use serde::Serialize;
use std::fs;

/// Save configuration to file.
///
/// # Arguments
/// * `config` - The configuration to save
///
/// # Returns
/// `Result<(), std::io::Error>` indicating success or failure
pub(crate) fn save_config(config: &Config) -> Result<(), std::io::Error> {
    // Local structs mirror the TOML layout; they live here so the public
    // Config type does not need to carry serde attributes it doesn't need
    // elsewhere.
    #[derive(Serialize)]
    struct TomlConfig {
        window: WindowConfig,
        search: SearchConfig,
        obsidian: Option<ObsidianConfig>,
        commands: Vec<CommandConfig>,
        theme: ThemeConfig,
    }

    #[derive(Serialize)]
    struct WindowConfig {
        width: i32,
        height: i32,
    }

    #[derive(Serialize)]
    struct SearchConfig {
        max_results: usize,
        app_dirs: Vec<String>,
        command_debounce_ms: u32,
        provider_blacklist: Vec<String>,
        workspace_bar_enabled: bool,
        pinned_apps: Vec<String>,
    }

    #[derive(Serialize)]
    struct ThemeConfig {
        mode: ThemeMode,
        custom_theme_path: Option<String>,
    }

    let toml_config = TomlConfig {
        window: WindowConfig {
            width: config.window_width,
            height: config.window_height,
        },
        search: SearchConfig {
            max_results: config.max_results,
            app_dirs: config.app_dirs.clone(),
            command_debounce_ms: config.command_debounce_ms,
            provider_blacklist: config.search_provider_blacklist.clone(),
            workspace_bar_enabled: config.workspace_bar_enabled,
            pinned_apps: config.pinned_apps.clone(),
        },
        obsidian: config.obsidian.clone(),
        commands: config.commands.clone(),
        theme: ThemeConfig {
            mode: config.theme,
            custom_theme_path: config.custom_theme_path.clone(),
        },
    };

    let toml_string = toml::to_string_pretty(&toml_config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let path = config::config_path();
    debug!("Saving configuration to {}", path.display());

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, toml_string)?;
    Ok(())
}
