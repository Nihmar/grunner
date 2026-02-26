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

const APP_ID: &str = "org.nihmar.grunner";

fn main() -> glib::ExitCode {
    let cfg = config::load();
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| {
        // GApplication ensures only one process runs at a time, but
        // connect_activate fires again on the existing process whenever a
        // second invocation is attempted. If a window already exists, bring
        // it to the front instead of building a second one.
        if let Some(win) = app.windows().first() {
            win.present();
            return;
        }
        ui::build_ui(app, &cfg);
    });
    app.run()
}
