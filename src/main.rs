use glib::ExitCode;
use grunner::{core, logging, providers, ui};
use gtk4::prelude::*;
use lexopt::prelude::*;
use libadwaita::Application;

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
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("grunner: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, lexopt::Error> {
    let mut parser = lexopt::Parser::from_env();
    let mut disable_modes = false;

    while let Some(arg) = parser.next()? {
        match arg {
            Short('h') | Long("help") => {
                print_help();
                return Ok(ExitCode::SUCCESS);
            }
            Short('v' | 'V') | Long("version") => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                return Ok(ExitCode::SUCCESS);
            }
            Short('s') | Long("simple") => {
                disable_modes = true;
            }
            Long("list-providers") => {
                print_providers();
                return Ok(ExitCode::SUCCESS);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    // GRUNNER_SIMPLE=1 also enables simple mode (recommended over -s since GTK may intercept it)
    disable_modes |= std::env::var("GRUNNER_SIMPLE").is_ok();

    // Initialize logging system
    if let Err(e) = logging::init() {
        eprintln!("Failed to initialize logging: {e}");
    }

    logging::setup_panic_hook();
    log::info!("Grunner {} starting up", env!("CARGO_PKG_VERSION"));

    let mut cfg = core::config::load();
    cfg.disable_modes = disable_modes;

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(move |app| {
        log::debug!("Application activated");

        let windows = app.windows();
        log::debug!("Number of windows: {}", windows.len());

        for (i, win) in windows.iter().enumerate() {
            let classes: Vec<String> = win.css_classes().iter().map(ToString::to_string).collect();
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

        ui::window::build_ui(app, &cfg);
    });

    Ok(app.run())
}

fn print_help() {
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
}

fn print_providers() {
    println!("Grunner Search Providers");
    println!("=======================\n");

    let providers = providers::dbus::discover_providers(&[]);
    println!("Found {} search provider(s):\n", providers.len());

    for (i, provider) in providers.iter().enumerate() {
        println!("{}. {}", i + 1, provider.desktop_id);
        println!("   Bus Name:       {}", provider.bus_name);
        println!("   Object Path:    {}", provider.object_path);
        println!("   App Icon:       {}", provider.app_icon);
        println!("   Default Disabled: {}", provider.default_disabled);
        println!();
    }

    let enabled_count = providers.iter().filter(|p| !p.default_disabled).count();
    println!("Summary:");
    println!("  Total providers: {}", providers.len());
    println!("  Enabled providers: {enabled_count}");
    println!(
        "  Default-disabled providers: {}",
        providers.len() - enabled_count
    );
}
