//! Obsidian tab — vault path (with folder-picker), daily notes folder,
//! new notes folder, and quick-note file.
//!
//! This tab is always visible since `config.obsidian` defaults to `Some(ObsidianConfig::default())`.

use super::make_tab_page;
use crate::core::config::Config;
use crate::utils::{contract_home, expand_home};
use glib::clone;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{EntryRow, PreferencesGroup};
use std::cell::RefCell;
use std::rc::Rc;

/// Append the "Obsidian" tab to `notebook`.
///
/// # Panics
/// Assumes `config_rc.borrow().obsidian` is `Some` — this is always the case
/// since `Config::default()` initializes it to `Some(ObsidianConfig::default())`.
#[allow(clippy::too_many_lines)]
pub fn build_tab(
    notebook: &gtk4::Notebook,
    config_rc: &Rc<RefCell<Config>>,
    parent: &libadwaita::ApplicationWindow,
) {
    let (scroll, inner) = make_tab_page();

    let obsidian_group = PreferencesGroup::builder()
        .title("Vault Configuration")
        .description("Configure Obsidian vault integration")
        .build();

    // ── Vault Path ───────────────────────────────────────────────────────────
    let vault_row = EntryRow::builder().title("Vault Path").build();
    vault_row.set_text(&config_rc.borrow().obsidian.as_ref().unwrap().vault);

    // Suffix button to open a folder picker
    let browse_button = gtk4::Button::from_icon_name("folder-open-symbolic");
    browse_button.set_css_classes(&["flat"]);
    browse_button.set_tooltip_text(Some("Browse for vault folder"));
    vault_row.add_suffix(&browse_button);

    vault_row.connect_changed({
        let config_rc = Rc::clone(config_rc);
        move |row| {
            if let Some(obs) = config_rc.borrow_mut().obsidian.as_mut() {
                obs.vault = row.text().to_string();
            }
        }
    });

    browse_button.connect_clicked({
        let config_rc = Rc::clone(config_rc);
        let vault_row = vault_row.clone();
        let parent = parent.clone();
        move |_| {
            let dialog = gtk4::FileChooserNative::builder()
                .title("Select Obsidian Vault Folder")
                .modal(true)
                .action(gtk4::FileChooserAction::SelectFolder)
                .transient_for(&parent)
                .build();

            // Pre-select the current vault path if it exists on disk
            let initial_folder = config_rc.borrow().obsidian.as_ref().and_then(|obs| {
                if obs.vault.is_empty() {
                    None
                } else {
                    let expanded = expand_home(&obs.vault);
                    if expanded.exists() {
                        Some(expanded)
                    } else {
                        None
                    }
                }
            });
            if let Some(folder) = initial_folder {
                let _ = dialog.set_current_folder(Some(&gtk4::gio::File::for_path(folder)));
            }

            dialog.connect_response(clone!(
                #[strong]
                config_rc,
                #[strong]
                vault_row,
                move |dialog, response| {
                    if response == gtk4::ResponseType::Accept
                        && let Some(file) = dialog.file()
                    {
                        let folder_path = file.path().unwrap_or_default();
                        // Store as tilde path for portability
                        let display_path = contract_home(&folder_path);
                        vault_row.set_text(&display_path);
                        if let Some(obs) = config_rc.borrow_mut().obsidian.as_mut() {
                            obs.vault = display_path;
                        }
                    }
                    dialog.destroy();
                }
            ));

            dialog.show();
        }
    });

    obsidian_group.add(&vault_row);

    // ── Daily Notes Folder ───────────────────────────────────────────────────
    let daily_row = EntryRow::builder().title("Daily Notes Folder").build();
    daily_row.set_text(
        &config_rc
            .borrow()
            .obsidian
            .as_ref()
            .unwrap()
            .daily_notes_folder,
    );
    daily_row.connect_changed({
        let config_rc = Rc::clone(config_rc);
        move |row| {
            if let Some(obs) = config_rc.borrow_mut().obsidian.as_mut() {
                obs.daily_notes_folder = row.text().to_string();
            }
        }
    });
    obsidian_group.add(&daily_row);

    // ── New Notes Folder ─────────────────────────────────────────────────────
    let new_row = EntryRow::builder().title("New Notes Folder").build();
    new_row.set_text(
        &config_rc
            .borrow()
            .obsidian
            .as_ref()
            .unwrap()
            .new_notes_folder,
    );
    new_row.connect_changed({
        let config_rc = Rc::clone(config_rc);
        move |row| {
            if let Some(obs) = config_rc.borrow_mut().obsidian.as_mut() {
                obs.new_notes_folder = row.text().to_string();
            }
        }
    });
    obsidian_group.add(&new_row);

    // ── Quick Note File ──────────────────────────────────────────────────────
    let quick_row = EntryRow::builder().title("Quick Note File").build();
    quick_row.set_text(&config_rc.borrow().obsidian.as_ref().unwrap().quick_note);
    quick_row.connect_changed({
        let config_rc = Rc::clone(config_rc);
        move |row| {
            if let Some(obs) = config_rc.borrow_mut().obsidian.as_mut() {
                obs.quick_note = row.text().to_string();
            }
        }
    });
    obsidian_group.add(&quick_row);

    inner.append(&obsidian_group);
    notebook.append_page(&scroll, Some(&gtk4::Label::new(Some("Obsidian"))));
}
