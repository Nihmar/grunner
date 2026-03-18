//! Theme settings tab for Grunner settings window

use crate::core::config::{Config, ThemeMode};
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

const THEMES: &[(ThemeMode, &str)] = &[
    (ThemeMode::System, "System (follows OS)"),
    (ThemeMode::SystemLight, "System Light"),
    (ThemeMode::SystemDark, "System Dark"),
    (ThemeMode::TokioNight, "Tokyo Night"),
    (ThemeMode::CatppuccinMocha, "Catppuccin Mocha"),
    (ThemeMode::CatppuccinLatte, "Catppuccin Latte"),
    (ThemeMode::Nord, "Nord"),
    (ThemeMode::GruvboxDark, "Gruvbox Dark"),
    (ThemeMode::GruvboxLight, "Gruvbox Light"),
    (ThemeMode::Dracula, "Dracula"),
    (ThemeMode::Custom, "Custom..."),
];

pub fn build_tab(notebook: &gtk4::Notebook, config_rc: &Rc<RefCell<Config>>) {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(12);
    page.set_margin_end(12);

    let label = gtk4::Label::new(Some("Theme"));
    notebook.append_page(&page, Some(&label));

    let theme_label = gtk4::Label::new(Some("Theme Mode"));
    theme_label.set_halign(gtk4::Align::Start);
    page.append(&theme_label);

    let theme_combo = gtk4::ComboBoxText::new();
    theme_combo.set_halign(gtk4::Align::Fill);
    theme_combo.set_hexpand(true);

    let current_theme = config_rc.borrow().theme;
    let mut current_index = 0;
    for (i, (mode, name)) in THEMES.iter().enumerate() {
        theme_combo.append_text(name);
        if *mode == current_theme {
            current_index = i as u32;
        }
    }
    theme_combo.set_active(Some(current_index));

    let config_rc_clone = Rc::clone(config_rc);
    theme_combo.connect_changed(move |combo| {
        if let Some(index) = combo.active()
            && let Some((mode, _)) = THEMES.get(index as usize)
        {
            config_rc_clone.borrow_mut().theme = *mode;
        }
    });

    page.append(&theme_combo);

    let path_label = gtk4::Label::new(Some("Custom Theme Path"));
    path_label.set_halign(gtk4::Align::Start);
    path_label.set_margin_top(12);
    page.append(&path_label);

    let path_entry = gtk4::Entry::new();
    path_entry.set_halign(gtk4::Align::Fill);
    path_entry.set_hexpand(true);
    path_entry.set_placeholder_text(Some("~/.config/grunner/themes/my_theme.css"));

    if let Some(ref path) = config_rc.borrow().custom_theme_path {
        path_entry.set_text(path);
    }

    let config_rc_clone2 = Rc::clone(config_rc);
    path_entry.connect_changed(move |entry| {
        let text = entry.text().to_string();
        if !text.is_empty() {
            config_rc_clone2.borrow_mut().custom_theme_path = Some(text);
        } else {
            config_rc_clone2.borrow_mut().custom_theme_path = None;
        }
    });

    page.append(&path_entry);

    let note = gtk4::Label::new(Some(
        "Note: Custom themes must define CSS custom properties for colors.\n\
         See grunner source for required variables.",
    ));
    note.add_css_class("muted");
    note.set_halign(gtk4::Align::Start);
    note.set_margin_top(12);
    note.set_wrap(true);
    page.append(&note);
}
