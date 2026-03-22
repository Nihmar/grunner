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

    let theme_names: Vec<&str> = THEMES.iter().map(|(_, name)| *name).collect();
    let model = gtk4::StringList::new(&theme_names);
    let current_theme = config_rc.borrow().theme;
    let current_index = THEMES
        .iter()
        .position(|(mode, _)| *mode == current_theme)
        .unwrap_or(0);

    let theme_combo = gtk4::DropDown::new(
        Some(model.clone().upcast::<gtk4::gio::ListModel>()),
        gtk4::Expression::NONE,
    );
    theme_combo.set_halign(gtk4::Align::Fill);
    theme_combo.set_hexpand(true);
    theme_combo.set_selected(u32::try_from(current_index).unwrap_or(0));

    let config_rc_clone = Rc::clone(config_rc);
    theme_combo.connect_selected_item_notify(move |dropdown| {
        if let Some(item) = dropdown
            .selected_item()
            .and_then(|i| i.downcast::<gtk4::StringObject>().ok())
        {
            if let Some(idx) = theme_names
                .iter()
                .position(|n| *n == item.string().as_str())
            {
                if let Some((mode, _)) = THEMES.get(idx) {
                    config_rc_clone.borrow_mut().theme = *mode;
                }
            }
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
        if text.is_empty() {
            config_rc_clone2.borrow_mut().custom_theme_path = None;
        } else {
            config_rc_clone2.borrow_mut().custom_theme_path = Some(text);
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
