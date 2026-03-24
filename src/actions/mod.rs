//! Action execution module for Grunner
//!
//! This module handles all external actions performed by the application:
//! - Launching applications (with or without terminal)
//! - Power management actions (logout, suspend, reboot, shutdown)
//! - File and line opening operations
//! - Obsidian vault and note management
//! - Settings management
//! - Workspace window operations

pub mod file;
pub mod launcher;
pub mod obsidian;
pub mod power;
pub mod settings;
pub mod workspace;

pub use file::*;
pub use launcher::*;
pub use obsidian::*;
pub use power::*;
pub use settings::*;

use gtk4::gio;
use gtk4::prelude::{ApplicationExt, DisplayExt};
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

/// Open a URI using GIO's default handler
///
/// # Arguments
/// * `uri` - The URI to open (obsidian://, http://, file://, etc.)
///
/// Uses `gio::AppInfo::launch_default_for_uri()` which handles the URI
/// via the correct application without spawning child processes under Grunner.
pub fn open_uri(uri: &str) -> Result<(), std::io::Error> {
    debug!("Opening URI: {uri}");
    let ctx = gtk4::gdk::Display::default().map(|d| d.app_launch_context());
    match gio::AppInfo::launch_default_for_uri(uri, ctx.as_ref()) {
        Ok(()) => {
            info!("Successfully opened URI: {uri}");
            Ok(())
        }
        Err(e) => {
            error!("Failed to open URI '{uri}': {e}");
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        }
    }
}
