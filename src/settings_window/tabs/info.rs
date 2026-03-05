//! Info tab — overview text, desktop ID reference, config file shortcut,
//! and the "reset to defaults" action.

use super::make_tab_page;
use crate::actions::open_uri;
use crate::config::{self, Config};
use crate::settings_window::save::save_config;
use gtk4::pango;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{PreferencesDialog, PreferencesGroup, PreferencesRow, Toast, ToastOverlay};
use log::{error, info};
use std::cell::RefCell;
use std::rc::Rc;

/// Append the "Info" tab to `notebook`.
pub fn build_tab(
    notebook: &gtk4::Notebook,
    config_rc: &Rc<RefCell<Config>>,
    window: &PreferencesDialog,
    overlay: &ToastOverlay,
) {
    let (scroll, inner) = make_tab_page();

    // ── How Grunner Works ────────────────────────────────────────────────────
    let explanation_group = PreferencesGroup::builder()
        .title("How Grunner Works")
        .description("Overview of features and usage")
        .build();

    let explanation_text = r#"Grunner is a GTK4 application launcher with advanced search capabilities and system integration.

## Default Search
Type any text to fuzzy-search all installed applications. Results are ranked by match score. The app's name, description, and icon are displayed in each row.

## Colon Commands
Type ':' followed by a command name and optional argument:

• :f <pattern> — Search your home directory for files
• :fg <pattern> — Search file contents recursively using ripgrep/grep
• :ob [text] — Obsidian actions (requires configuration)
• :obg <pattern> — Search Obsidian vault content

## Obsidian Action Bar
When you type `:ob` (with optional text), an action bar appears with four buttons:

• Open Vault — Opens your configured Obsidian vault
• New Note — Creates a timestamped note in the new notes folder
• Daily Note — Opens or creates today's daily note
• Quick Note — Appends text to the quick note file

Selecting any button performs the corresponding action.

## Power Bar Buttons
At the bottom of the main window:
• Settings (left) — Open this settings dialog
• Suspend — Put system to sleep (with confirmation)
• Restart — Reboot system (with confirmation)
• Power off — Shut down system (with confirmation)
• Log out — End current user session

## Search Providers
Grunner integrates with GNOME Shell search providers (Files, Calendar, Contacts, etc.) for unified searching."#;

    let explanation_label = gtk4::Label::builder()
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .selectable(false)
        .xalign(0.0)
        .yalign(0.0)
        .hexpand(true)
        .build();
    explanation_label.set_text(explanation_text);
    explanation_label.add_css_class("explanation-label");

    let explanation_row = PreferencesRow::new();
    explanation_row.set_child(Some(&explanation_label));
    explanation_group.add(&explanation_row);
    inner.append(&explanation_group);

    // ── Desktop IDs for Blacklist ────────────────────────────────────────────
    let desktop_ids_group = PreferencesGroup::builder()
        .title("Desktop IDs for Blacklist")
        .description("Common GNOME desktop IDs you can copy and paste into the blacklist")
        .build();

    let desktop_ids = vec![
        "org.gnome.Contacts.desktop",
        "org.gnome.Calculator.desktop",
        "org.gnome.Characters.desktop",
        "org.gnome.Epiphany.desktop",
        "org.gnome.Weather.desktop",
        "org.gnome.Software.desktop",
        "org.gnome.Settings.desktop",
        "org.gnome.Calendar.desktop",
        "org.gnome.clocks.desktop",
        "org.gnome.Nautilus.desktop",
    ];

    let ids_text = gtk4::TextView::builder()
        .wrap_mode(gtk4::WrapMode::WordChar)
        .editable(false)
        .cursor_visible(false)
        .build();
    ids_text.buffer().set_text(&desktop_ids.join("\n"));

    let ids_scrolled = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .min_content_height(120)
        .max_content_height(200)
        .build();
    ids_scrolled.set_child(Some(&ids_text));

    let ids_row = PreferencesRow::new();
    ids_row.set_child(Some(&ids_scrolled));
    desktop_ids_group.add(&ids_row);
    inner.append(&desktop_ids_group);

    // ── Configuration File ───────────────────────────────────────────────────
    let config_group = PreferencesGroup::builder()
        .title("Configuration File")
        .description("Open the configuration file directly in your default editor")
        .build();

    let config_button = gtk4::Button::builder().label("Open Config File").build();
    config_button.connect_clicked({
        let window = window.downgrade();
        let overlay = overlay.downgrade();
        move |_| {
            let config_path = config::config_path();
            let uri = format!("file://{}", config_path.to_string_lossy());
            if let Err(e) = open_uri(&uri) {
                error!("Failed to open config file: {}", e);
                if let Some(overlay) = overlay.upgrade() {
                    let toast = Toast::builder()
                        .title("Failed to open config file")
                        .timeout(3)
                        .build();
                    overlay.add_toast(toast);
                }
            } else {
                info!("Opened config file: {}", uri);
                // Close the settings window after opening config
                if let Some(window) = window.upgrade() {
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(500),
                        move || {
                            libadwaita::prelude::AdwDialogExt::close(&window);
                        },
                    );
                }
            }
        }
    });
    config_group.add(&config_button);
    inner.append(&config_group);

    // ── Reset to Defaults ────────────────────────────────────────────────────
    let reset_group = PreferencesGroup::builder()
        .title("Reset to Defaults")
        .description("Reset all settings to their default values")
        .build();

    let reset_button = gtk4::Button::builder().label("Reset All Settings").build();
    reset_button.add_css_class("destructive-action");
    reset_button.connect_clicked({
        let window = window.downgrade();
        let overlay = overlay.downgrade();
        let config_rc = Rc::clone(config_rc);
        move |_| {
            let default_config = Config::default();
            {
                let mut cfg = config_rc.borrow_mut();
                cfg.window_width = default_config.window_width;
                cfg.window_height = default_config.window_height;
                cfg.max_results = default_config.max_results;
                cfg.command_debounce_ms = default_config.command_debounce_ms;
                cfg.app_dirs = default_config.app_dirs.clone();
                cfg.search_provider_blacklist = default_config.search_provider_blacklist.clone();
                cfg.obsidian = default_config.obsidian;
            }

            if let Some(window) = window.upgrade() {
                if let Some(overlay) = overlay.upgrade() {
                    if let Err(e) = save_config(&config_rc.borrow()) {
                        error!("Failed to save reset configuration: {}", e);
                        let toast = Toast::builder()
                            .title("Failed to save reset settings")
                            .timeout(3)
                            .build();
                        overlay.add_toast(toast);
                    } else {
                        info!("Configuration reset and saved successfully");
                        let toast = Toast::builder()
                            .title("Settings reset to defaults")
                            .timeout(2)
                            .build();
                        overlay.add_toast(toast);
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(1000),
                            move || {
                                libadwaita::prelude::AdwDialogExt::close(&window);
                            },
                        );
                    }
                }
            }
        }
    });
    reset_group.add(&reset_button);
    inner.append(&reset_group);

    notebook.append_page(&scroll, Some(&gtk4::Label::new(Some("Info"))));
}
