mod actions;
mod app_item;
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
use gtk4::prelude::*;
use libadwaita::Application;
mod bookmark_item;
mod bookmarks;
mod clipboard_history;
mod clipboard_item;
mod history;

const APP_ID: &str = "org.nihmar.grunner";

fn main() -> glib::ExitCode {
    let cfg = config::load();
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| {
        ui::build_ui(app, &cfg);
    });
    app.run()
}
