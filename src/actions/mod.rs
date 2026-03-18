//! Action execution module for Grunner
//!
//! This module handles all external actions performed by the application:
//! - Launching applications (with or without terminal)
//! - Power management actions (logout, suspend, reboot, shutdown)
//! - File and line opening operations
//! - Obsidian vault and note management
//! - Settings management

pub mod file;
pub mod launcher;
pub mod obsidian;
pub mod power;
pub mod settings;

pub use file::*;
pub use launcher::*;
pub use obsidian::*;
pub use power::*;
pub use settings::*;

use gtk4::prelude::ApplicationExt;
use log::{debug, error, info};

/// Show an error notification to the user
pub fn show_error_notification(message: &str) {
    if gtk4::gdk::Display::default().is_some() {
        let notification = gtk4::gio::Notification::new("Launch Failed");
        notification.set_body(Some(message));
        if let Some(app) = gtk4::gio::Application::default() {
            app.send_notification(Some("launch-error"), &notification);
        }
    }
}

/// Open a URI using xdg-open
///
/// # Arguments
/// * `uri` - The URI to open (obsidian://, http://, etc.)
///
/// # Errors
/// Returns an error if xdg-open fails to spawn or execute.
///
/// Uses the system's default URI handler (xdg-open on Linux) to open the URI.
pub fn open_uri(uri: &str) -> Result<(), std::io::Error> {
    debug!("Opening URI: {uri}");
    match std::process::Command::new("xdg-open").arg(uri).spawn() {
        Ok(_) => {
            info!("Successfully opened URI: {uri}");
            Ok(())
        }
        Err(e) => {
            error!("Failed to open URI '{uri}': {e}");
            Err(e)
        }
    }
}
