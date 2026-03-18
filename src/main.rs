mod actions;
mod app_mode;
mod calculator;
mod command_handler;
mod core {
    pub mod config;
    pub mod global_state;
    pub mod theme;
}
mod item_activation;
mod launcher;
mod logging;
mod model {
    pub mod items;
    pub mod list_model;
}
mod providers;
mod settings_window;
mod ui {
    pub mod list_factory;
    pub mod obsidian_bar;
    pub mod power_bar;
    pub mod window;
    pub mod workspace_bar;
}
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
    if args.contains(&"--version".to_string())
        || args.contains(&"-v".to_string())
        || args.contains(&"-V".to_string())
    {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    // Show help
    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        println!("grunner - a fast, keyboard-driven application launcher");
        println!();
        println!("Usage: grunner [OPTIONS]");
        println!();
        println!("Options:");
        println!("  -h, --help            Show this help message");
        println!("  -v, --version         Show version information");
        println!("  -s, --simple          Simple mode: only app search, hide power bar");
        println!("      --list-providers  List available GNOME Shell search providers");
        println!();
        println!("Environment variables:");
        println!("  GRUNNER_SIMPLE=1      Enable simple mode (recommended, more reliable than -s)");
        return ExitCode::SUCCESS;
    }

    // Check for simple mode flag (--simple) or environment variable
    // Note: GTK may intercept -s, so GRUNNER_SIMPLE env var is recommended
    let disable_modes = args.iter().any(|a| a == "-s" || a == "--simple")
        || std::env::var("GRUNNER_SIMPLE").is_ok();

    // Handle list-providers flag requests
    if args.contains(&"--list-providers".to_string()) {
        println!("Grunner Search Providers");
        println!("=======================\n");

        let providers = providers::dbus_provider::discover_providers(&[]);
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
        println!("  Enabled providers: {enabled_count}");
        println!(
            "  Default-disabled providers: {}",
            providers.len() - enabled_count
        );

        return ExitCode::SUCCESS;
    }

    // Initialize logging system
    if let Err(e) = logging::init() {
        eprintln!("Failed to initialize logging: {e}");
        // Continue without logging
    }

    // Set up panic hook to log panics
    logging::setup_panic_hook();

    // Log application startup
    log::info!("Grunner {} starting up", env!("CARGO_PKG_VERSION"));

    // Load application configuration from file
    let mut cfg = core::config::load();

    // Apply command-line flags
    cfg.disable_modes = disable_modes;

    // Create the GTK application with the specified application ID
    let app = Application::builder().application_id(APP_ID).build();

    // Connect the activation signal to build the UI when the app starts
    app.connect_activate(move |app| {
        log::debug!("Application activated");

        // Find existing launcher window (identified by CSS class "launcher-window")
        let windows = app.windows();
        log::debug!("Number of windows: {}", windows.len());

        // Debug: print all windows and their CSS classes
        for (i, win) in windows.iter().enumerate() {
            let classes: Vec<String> = win.css_classes().iter().map(|c| c.to_string()).collect();
            log::debug!(
                "Window {}: visible={}, CSS classes: {:?}",
                i,
                win.is_visible(),
                classes
            );
        }

        let launcher_window = windows
            .iter()
            .find(|win| win.css_classes().iter().any(|c| c == "launcher-window"));

        if let Some(win) = launcher_window {
            log::debug!("Found launcher window, visible: {}", win.is_visible());

            // Toggle visibility of existing launcher window
            if win.is_visible() {
                log::debug!("Hiding window");

                win.hide();
            } else {
                log::debug!("Presenting window");

                win.present();
            }
            return;
        }
        log::debug!("No launcher window found, building new UI");

        // No launcher window exists - build the main user interface
        ui::window::build_ui(app, &cfg);
    });

    // Run the GTK application main loop
    app.run()
}
