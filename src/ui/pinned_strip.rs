//! Pinned applications strip for Grunner

use crate::actions::launch_app;
use crate::core::config;
use crate::launcher::DesktopApp;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, EventControllerMotion, Image, Orientation, Overlay};
use log::{error, info};
use std::cell::RefCell;
use std::rc::Rc;

/// Build the pinned apps strip container (vertical layout for right sidebar)
pub fn build_pinned_strip() -> GtkBox {
    let strip = GtkBox::new(Orientation::Vertical, 6);
    strip.set_valign(Align::Start);
    strip.add_css_class("pinned-strip");
    strip.set_visible(false);
    strip
}

fn build_remove_badge() -> Button {
    let badge = Button::builder()
        .icon_name("list-remove-symbolic")
        .halign(Align::End)
        .valign(Align::Start)
        .build();
    badge.add_css_class("pinned-remove-badge");
    badge.set_visible(false);
    badge
}

/// Update the pinned strip buttons based on current config and loaded apps
pub fn update_pinned_strip(
    strip: &GtkBox,
    pinned_apps: &[String],
    loaded_apps: &[DesktopApp],
    window: &libadwaita::ApplicationWindow,
    pinned_apps_ref: &Rc<RefCell<Vec<String>>>,
    all_apps_ref: &Rc<RefCell<Vec<DesktopApp>>>,
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

            // Overlay with remove badge (appears on hover)
            let overlay = Overlay::new();
            overlay.set_child(Some(&btn));

            let remove_badge = build_remove_badge();
            overlay.add_overlay(&remove_badge);

            let motion = EventControllerMotion::new();
            motion.connect_enter(clone!(
                #[weak]
                remove_badge,
                move |_, _, _| {
                    remove_badge.set_visible(true);
                }
            ));
            motion.connect_leave(clone!(
                #[weak]
                remove_badge,
                move |_| {
                    remove_badge.set_visible(false);
                }
            ));
            overlay.add_controller(motion);

            let did = desktop_id.clone();
            let app_name = app.name.clone();
            let p_apps = pinned_apps_ref.clone();
            let p_all = all_apps_ref.clone();
            let p_strip = strip.clone();
            let win_remove = window.clone();
            remove_badge.connect_clicked(move |_| {
                {
                    let mut pinned = p_apps.borrow_mut();
                    pinned.retain(|d| d != &did);
                    info!("Removed from Favorites: {app_name}");
                }
                save_pinned_apps(&p_apps.borrow());
                refresh_pinned_strip(&p_strip, &p_apps, &p_all, &win_remove);
            });

            strip.append(&overlay);
        }
    }
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
pub fn update_strip_visibility(strip: &GtkBox, pinned_apps: &[String], query_is_empty: bool) {
    let visible = !pinned_apps.is_empty() && query_is_empty;
    strip.set_visible(visible);
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
    pinned_apps: &Rc<RefCell<Vec<String>>>,
    all_apps: &Rc<RefCell<Vec<DesktopApp>>>,
    window: &libadwaita::ApplicationWindow,
) {
    let pinned = pinned_apps.borrow();
    let apps = all_apps.borrow();
    update_pinned_strip(strip, &pinned, &apps, window, pinned_apps, all_apps);
    update_strip_visibility(strip, &pinned, true);
}
