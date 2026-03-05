//! Search tab — result limits, debounce delay, app directories,
//! and the search-provider blacklist.

use super::make_tab_page;
use crate::config::Config;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, PreferencesRow, SpinRow};
use std::cell::RefCell;
use std::rc::Rc;

/// Append the "Search" tab to `notebook`.
pub fn build_tab(notebook: &gtk4::Notebook, config_rc: &Rc<RefCell<Config>>) {
    let (scroll, inner) = make_tab_page();

    // ── Behavior ─────────────────────────────────────────────────────────────
    let behavior_group = PreferencesGroup::builder()
        .title("Behavior")
        .description("Configure how search results are displayed")
        .build();

    let max_results_row = SpinRow::builder()
        .title("Maximum Results")
        .subtitle("Maximum number of search results to display")
        .build();
    max_results_row.set_range(10.0, 200.0);
    max_results_row.adjustment().set_step_increment(1.0);
    max_results_row.adjustment().set_page_increment(10.0);
    max_results_row.set_value(config_rc.borrow().max_results as f64);
    max_results_row.connect_notify_local(Some("value"), {
        let config_rc = Rc::clone(config_rc);
        move |row, _| {
            config_rc.borrow_mut().max_results = row.value() as usize;
        }
    });
    behavior_group.add(&max_results_row);

    let debounce_row = SpinRow::builder()
        .title("Command Debounce")
        .subtitle("Delay before executing colon commands (milliseconds)")
        .build();
    debounce_row.set_range(100.0, 2000.0);
    debounce_row.adjustment().set_step_increment(50.0);
    debounce_row.adjustment().set_page_increment(100.0);
    debounce_row.set_value(config_rc.borrow().command_debounce_ms as f64);
    debounce_row.connect_notify_local(Some("value"), {
        let config_rc = Rc::clone(config_rc);
        move |row, _| {
            config_rc.borrow_mut().command_debounce_ms = row.value() as u32;
        }
    });
    behavior_group.add(&debounce_row);
    inner.append(&behavior_group);

    // ── Application Directories ──────────────────────────────────────────────
    let dirs_group = PreferencesGroup::builder()
        .title("Application Directories")
        .description("Paths scanned for .desktop files (one per line)")
        .build();

    let dirs_text = gtk4::TextView::builder()
        .wrap_mode(gtk4::WrapMode::WordChar)
        .build();
    let dirs_buffer = dirs_text.buffer();
    dirs_buffer.set_text(
        &config_rc
            .borrow()
            .app_dirs
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    dirs_buffer.connect_changed({
        let config_rc = Rc::clone(config_rc);
        move |buffer| {
            let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            config_rc.borrow_mut().app_dirs = text
                .split('\n')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .map(|s| crate::utils::expand_home(&s))
                .collect();
        }
    });

    let dirs_scrolled = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .min_content_height(100)
        .max_content_height(200)
        .build();
    dirs_scrolled.set_child(Some(&dirs_text));

    let dirs_row = PreferencesRow::new();
    dirs_row.set_child(Some(&dirs_scrolled));
    dirs_group.add(&dirs_row);
    inner.append(&dirs_group);

    // ── Search Provider Blacklist ────────────────────────────────────────────
    let blacklist_group = PreferencesGroup::builder()
        .title("Search Provider Blacklist")
        .description("List of GNOME Shell search providers to exclude (one per line)")
        .build();

    let blacklist_text = gtk4::TextView::builder()
        .wrap_mode(gtk4::WrapMode::WordChar)
        .build();
    let blacklist_buffer = blacklist_text.buffer();
    blacklist_buffer.set_text(&config_rc.borrow().search_provider_blacklist.join("\n"));
    blacklist_buffer.connect_changed({
        let config_rc = Rc::clone(config_rc);
        move |buffer| {
            let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            config_rc.borrow_mut().search_provider_blacklist = text
                .split('\n')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    });

    let blacklist_scrolled = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .min_content_height(60)
        .max_content_height(120)
        .build();
    blacklist_scrolled.set_child(Some(&blacklist_text));

    let blacklist_row = PreferencesRow::new();
    blacklist_row.set_child(Some(&blacklist_scrolled));
    blacklist_group.add(&blacklist_row);
    inner.append(&blacklist_group);

    notebook.append_page(&scroll, Some(&gtk4::Label::new(Some("Search"))));
}
