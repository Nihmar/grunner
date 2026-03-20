//! Configuration persistence for the settings window.
//!
//! Kept separate from the UI code so serialisation logic can be
//! read and tested independently of GTK.

use crate::core::config::{self, config_to_toml, Config};
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
