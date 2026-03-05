mod actions;
mod app_mode;
mod config;
mod item_activation;
mod items;
mod launcher;
mod list_model;
mod logging;
mod obsidian_bar;
mod power_bar;
mod search_provider;
mod settings_window;
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

    // Handle list-providers flag requests
    if args.contains(&"--list-providers".to_string()) {
        println!("Grunner Search Providers");
        println!("=======================\n");

        let providers = search_provider::discover_providers(&[]);
        println!("Found {} search provider(s):\n", providers.len());

        for (i, provider) in providers.iter().enumerate() {
            println!("{}. {}", i + 1, provider.desktop_id);
            println!("   Bus Name:       {}", provider.bus_name);
            println!("   Object Path:    {}", provider.object_path);
            println!("   App Icon:       {}", provider.app_icon);
            println!("   Default Disabled: {}", provider.default_disabled);
            println!();
        }

        // Print summary
        let enabled_count = providers.iter().filter(|p| !p.default_disabled).count();
        println!("Summary:");
        println!("  Total providers: {}", providers.len());
        println!("  Enabled providers: {}", enabled_count);
        println!(
            "  Default-disabled providers: {}",
            providers.len() - enabled_count
        );

        return ExitCode::SUCCESS;
    }

    // Initialize logging system
    if let Err(e) = logging::init() {
        eprintln!("Failed to initialize logging: {}", e);
        // Continue without logging
    }

    // Set up panic hook to log panics
    logging::setup_panic_hook();

    // Log application startup
    log::info!("Grunner {} starting up", env!("CARGO_PKG_VERSION"));

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
