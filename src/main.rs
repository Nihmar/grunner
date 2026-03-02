mod actions;
mod app_item;
mod app_mode;
mod cmd_item;
mod config;
mod item_activation;
mod launcher;
mod list_model;
mod obsidian_bar;
mod obsidian_item;
mod power_bar;
mod search_provider;
mod search_result_item;
mod ui;
mod utils;

use glib::ExitCode;
use gtk4::prelude::*;
use libadwaita::Application;
use std::env;

/// Application ID for D-Bus and GNOME Shell integration
const APP_ID: &str = "org.nihmar.grunner";

/// Main entry point for the Grunner application
///
/// Grunner is a GTK4 application launcher with Obsidian integration and power controls.
/// This function:
/// 1. Parses command-line arguments
/// 2. Loads configuration
/// 3. Creates and runs the GTK application
///
/// Returns: `ExitCode::SUCCESS` on normal execution
fn main() -> glib::ExitCode {
    // Parse command-line arguments for version flag
    let args: Vec<String> = env::args().collect();

    // Handle version flag requests
    if args.contains(&"--version".to_string()) || args.contains(&"-V".to_string()) {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    // Load application configuration from file
    let cfg = config::load();

    // Create the GTK application with the specified application ID
    let app = Application::builder().application_id(APP_ID).build();

    // Connect the activation signal to build the UI when the app starts
    app.connect_activate(move |app| {
        // If a window already exists, present it instead of creating a new one
        if let Some(win) = app.windows().first() {
            win.present();
            return;
        }
        // Build the main user interface with the loaded configuration
        ui::build_ui(app, &cfg);
    });

    // Run the GTK application main loop
    app.run()
}
