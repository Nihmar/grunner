mod actions;
mod app_item;
mod app_mode;
mod calc_item;
mod calculator;
mod cmd_item;
mod config;
mod launcher;
mod list_model;
mod obsidian_item;
mod search_provider;
mod search_result_item;
mod ui;
mod utils;
use glib::ExitCode;
use gtk4::prelude::*;
use libadwaita::Application;
use std::env;

const APP_ID: &str = "org.nihmar.grunner";

fn main() -> glib::ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.contains(&"--version".to_string()) || args.contains(&"-V".to_string()) {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let cfg = config::load();
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| {
        if let Some(win) = app.windows().first() {
            win.present();
            return;
        }
        ui::build_ui(app, &cfg);
    });
    app.run()
}
