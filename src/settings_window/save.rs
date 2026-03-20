//! Configuration persistence for the settings window.
//!
//! Kept separate from the UI code so serialisation logic can be
//! read and tested independently of GTK.

use crate::core::config::{self, Config, config_to_toml};
use log::debug;
use std::fs;

/// Save configuration to file.
///
/// # Arguments
/// * `config` - The configuration to save
///
/// # Returns
/// `Result<(), std::io::Error>` indicating success or failure
pub(crate) fn save_config(config: &Config) -> Result<(), std::io::Error> {
    let toml_string = config_to_toml(config);

    let path = config::config_path();
    debug!("Saving configuration to {}", path.display());

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, toml_string)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Config;

    #[test]
    fn test_config_to_toml_produces_valid_output() {
        let config = Config::default();
        let toml_str = config_to_toml(&config);
        // Should be parseable as TOML
        let parsed: Result<toml::Value, _> = toml::from_str(&toml_str);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_config_to_toml_contains_all_sections() {
        let config = Config::default();
        let toml_str = config_to_toml(&config);
        assert!(toml_str.contains("[window]"));
        assert!(toml_str.contains("[search]"));
        assert!(toml_str.contains("[theme]"));
        // Empty commands may be omitted or represented as empty array
        let parsed: toml::Value = toml::from_str(&toml_str).unwrap();
        assert!(parsed.get("window").is_some());
        assert!(parsed.get("search").is_some());
        assert!(parsed.get("theme").is_some());
    }

    #[test]
    fn test_config_to_toml_round_trip_via_toml_crate() {
        let mut config = Config::default();
        config.window_width = 1280;
        config.window_height = 720;

        let toml_str = config_to_toml(&config);
        let value: toml::Value = toml::from_str(&toml_str).unwrap();

        assert_eq!(value["window"]["width"].as_integer().unwrap(), 1280);
        assert_eq!(value["window"]["height"].as_integer().unwrap(), 720);
    }
}
