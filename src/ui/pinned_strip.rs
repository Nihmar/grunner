//! Pinned applications strip for Grunner

use crate::actions::launch_app;
use crate::core::config;
use crate::launcher::DesktopApp;
use glib::clone;
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Entry, EventControllerKey, GestureClick, Image, Orientation, Popover};
use log::{error, info};
use std::cell::RefCell;
use std::rc::Rc;

/// Build the pinned apps strip container and separator
pub fn build_pinned_strip() -> (GtkBox, GtkBox) {
    let strip = GtkBox::new(Orientation::Horizontal, 4);
    strip.set_halign(Align::Center);
    strip.add_css_class("pinned-strip");
    strip.set_visible(false);

    let separator = GtkBox::new(Orientation::Horizontal, 0);
    separator.set_hexpand(true);
    separator.add_css_class("pinned-separator");
    separator.set_visible(false);

    (strip, separator)
}

/// Update the pinned strip buttons based on current config and loaded apps
#[allow(clippy::too_many_arguments)]
pub fn update_pinned_strip(
    strip: &GtkBox,
    pinned_apps: &[String],
    loaded_apps: &[DesktopApp],
    window: &libadwaita::ApplicationWindow,
    pinned_apps_ref: &Rc<RefCell<Vec<String>>>,
    all_apps_ref: &Rc<RefCell<Vec<DesktopApp>>>,
    pinned_strip_ref: &GtkBox,
    pinned_separator_ref: &GtkBox,
    entry: &Entry,
) {
    while let Some(child) = strip.first_child() {
        strip.remove(&child);
    }

    if pinned_apps.is_empty() {
        return;
    }

    for desktop_id in pinned_apps {
        if let Some(app) = loaded_apps.iter().find(|a| a.desktop_id == *desktop_id) {
            let btn = Button::new();
            btn.set_focusable(false);
            btn.add_css_class("pinned-button");

            let icon = Image::new();
            icon.set_pixel_size(30);
            icon.set_valign(Align::Center);
            if app.icon.is_empty() {
                icon.set_icon_name(Some("application-x-executable"));
            } else if app.icon.starts_with('/') {
                icon.set_from_file(Some(&app.icon));
            } else {
                icon.set_icon_name(Some(&app.icon));
            }
            btn.set_child(Some(&icon));
            btn.set_tooltip_text(Some(&app.name));

            // Left-click: launch app and hide window
            let exec = app.exec.clone();
            let terminal = app.terminal;
            let win_click = window.clone();
            btn.connect_clicked(move |_| {
                info!("Launching pinned app: {exec}");
                launch_app(&exec, terminal, None);
                win_click.hide();
            });

            // Right-click: show popover with Open / Remove buttons
            let did = desktop_id.clone();
            let app_name = app.name.clone();
            let app_exec = app.exec.clone();
            let app_terminal = app.terminal;
            let win_ctx = window.clone();
            let p_apps = pinned_apps_ref.clone();
            let p_all = all_apps_ref.clone();
            let p_strip = pinned_strip_ref.clone();
            let p_sep = pinned_separator_ref.clone();
            let entry_ctx = entry.clone();

            let right_click = GestureClick::new();
            right_click.set_button(3);
            right_click.connect_pressed(clone!(
                #[weak]
                btn,
                move |_gesture, _n_press, _x, _y| {
                    let popover = build_pinned_popover(
                        &btn,
                        &did,
                        &app_name,
                        &app_exec,
                        app_terminal,
                        &win_ctx,
                        &p_apps,
                        &p_all,
                        &p_strip,
                        &p_sep,
                        &entry_ctx,
                    );
                    popover.popup();
                }
            ));
            btn.add_controller(right_click);

            strip.append(&btn);
        }
    }
}

/// Build a popover with Open and Remove buttons for a pinned app
#[allow(clippy::too_many_arguments)]
fn build_pinned_popover(
    parent: &Button,
    desktop_id: &str,
    app_name: &str,
    exec: &str,
    terminal: bool,
    window: &libadwaita::ApplicationWindow,
    pinned_apps: &Rc<RefCell<Vec<String>>>,
    all_apps: &Rc<RefCell<Vec<DesktopApp>>>,
    pinned_strip: &GtkBox,
    pinned_separator: &GtkBox,
    entry: &Entry,
) -> Popover {
    let popover = Popover::new();
    popover.set_parent(parent);
    popover.set_has_arrow(true);
    let popover_ref = RefCell::new(Some(popover.clone()));

    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.add_css_class("pinned-popover-menu");

    // Open button
    let open_btn = Button::new();
    open_btn.set_label("Open");
    open_btn.add_css_class("flat");
    open_btn.set_hexpand(true);
    open_btn.set_halign(Align::Fill);
    let exec_open = exec.to_string();
    let win_open = window.clone();
    open_btn.connect_clicked(move |_| {
        info!("Launching pinned app from menu: {exec_open}");
        launch_app(&exec_open, terminal, None);
        win_open.hide();
    });
    vbox.append(&open_btn);

    // Remove button
    let remove_btn = Button::new();
    remove_btn.set_label("Remove from Favorites");
    remove_btn.add_css_class("flat");
    remove_btn.set_hexpand(true);
    remove_btn.set_halign(Align::Fill);
    let did_remove = desktop_id.to_string();
    let name_remove = app_name.to_string();
    let p_apps = pinned_apps.clone();
    let p_all = all_apps.clone();
    let p_strip = pinned_strip.clone();
    let p_sep = pinned_separator.clone();
    let win_remove = window.clone();
    let p_ref = popover_ref.clone();
    let entry_remove = entry.clone();
    remove_btn.connect_clicked(move |_| {
        {
            let mut pinned = p_apps.borrow_mut();
            pinned.retain(|d| d != &did_remove);
            info!("Removed from Favorites: {name_remove}");
        }
        save_pinned_apps(&p_apps.borrow());
        refresh_pinned_strip(
            &p_strip,
            &p_sep,
            &p_apps,
            &p_all,
            &win_remove,
            &entry_remove,
        );
        if let Some(p) = p_ref.borrow().as_ref() {
            p.popdown();
        }
        entry_remove.grab_focus();
    });
    vbox.append(&remove_btn);

    popover.set_child(Some(&vbox));

    let popover_esc = popover.clone();
    let entry_esc = entry.clone();
    let key_ctrl = EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            popover_esc.popdown();
            entry_esc.grab_focus();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    popover.add_controller(key_ctrl);

    popover
}

/// Launch the N-th pinned app (0-indexed, for Alt+1..Alt+9)
pub fn launch_pinned_by_index(
    index: usize,
    pinned_apps: &[String],
    loaded_apps: &[DesktopApp],
    window: &libadwaita::ApplicationWindow,
) {
    if let Some(desktop_id) = pinned_apps.get(index)
        && let Some(app) = loaded_apps.iter().find(|a| a.desktop_id == *desktop_id)
    {
        info!("Launching pinned app #{index}: {}", app.name);
        launch_app(&app.exec, app.terminal, None);
        window.hide();
    }
}

/// Update strip visibility based on pinned apps and search query
pub fn update_strip_visibility(
    strip: &GtkBox,
    separator: &GtkBox,
    pinned_apps: &[String],
    query_is_empty: bool,
) {
    let visible = !pinned_apps.is_empty() && query_is_empty;
    strip.set_visible(visible);
    separator.set_visible(visible);
}

/// Save pinned apps list to the config file on disk
pub fn save_pinned_apps(pinned_apps: &[String]) {
    let mut cfg = config::load();
    cfg.pinned_apps = pinned_apps.to_vec();
    if let Err(e) = crate::settings_window::save::save_config(&cfg) {
        error!("Failed to save pinned apps: {e}");
    }
}

/// Refresh the pinned strip after add/remove — rebuilds buttons from current state
pub fn refresh_pinned_strip(
    strip: &GtkBox,
    separator: &GtkBox,
    pinned_apps: &Rc<RefCell<Vec<String>>>,
    all_apps: &Rc<RefCell<Vec<DesktopApp>>>,
    window: &libadwaita::ApplicationWindow,
    entry: &Entry,
) {
    let pinned = pinned_apps.borrow();
    let apps = all_apps.borrow();
    update_pinned_strip(
        strip,
        &pinned,
        &apps,
        window,
        pinned_apps,
        all_apps,
        strip,
        separator,
        entry,
    );
    update_strip_visibility(strip, separator, &pinned, true);
}
