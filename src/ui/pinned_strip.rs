//! Pinned applications strip for Grunner

use crate::actions::launch_app;
use crate::core::config;
use crate::launcher::DesktopApp;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, DragSource, DropTarget, EventControllerMotion, GestureClick,
    Image, Orientation, Overlay, gdk_pixbuf,
};
use log::{error, info};
use std::cell::{Cell, RefCell};
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

fn clear_drag_target_class(strip: &GtkBox) {
    let mut child = strip.first_child();
    while let Some(widget) = child {
        widget.remove_css_class("drag-target");
        child = widget.next_sibling();
    }
}

fn clear_all_drag_classes(strip: &GtkBox) {
    let mut child = strip.first_child();
    while let Some(widget) = child {
        widget.remove_css_class("drag-source");
        widget.remove_css_class("drag-target");
        child = widget.next_sibling();
    }
}

/// Setup drag source and drop target on an overlay for reordering pinned apps
fn setup_drag_and_drop(
    overlay: &Overlay,
    desktop_id: &str,
    strip: &GtkBox,
    pinned_apps_ref: &Rc<RefCell<Vec<String>>>,
    drag_source_id: &Rc<RefCell<Option<String>>>,
    dragging: &Rc<Cell<bool>>,
) {
    let did = desktop_id.to_string();

    // DragSource: initiates reorder drag
    let drag_source = DragSource::new();
    let src_id_prepare = drag_source_id.clone();
    let did_prepare = did.clone();
    drag_source.connect_prepare(move |_, _, _| {
        *src_id_prepare.borrow_mut() = Some(did_prepare.clone());
        Some(gtk4::gdk::ContentProvider::for_value(&glib::Value::from(
            &did_prepare as &str,
        )))
    });

    let overlay_begin = overlay.clone();
    let dragging_begin = dragging.clone();
    drag_source.connect_drag_begin(move |src, _drag| {
        overlay_begin.add_css_class("drag-source");
        dragging_begin.set(true);
        // Set a tiny transparent pixbuf as drag icon so no text renders
        let transparent = gtk4::gdk::Texture::for_pixbuf(
            &gdk_pixbuf::Pixbuf::new(gdk_pixbuf::Colorspace::Rgb, true, 8, 1, 1).unwrap(),
        );
        src.set_icon(Some(&transparent), 0, 0);
    });

    let src_id_end = drag_source_id.clone();
    let strip_end = strip.clone();
    let dragging_end = dragging.clone();
    drag_source.connect_drag_end(move |_, _, _| {
        *src_id_end.borrow_mut() = None;
        dragging_end.set(false);
        clear_all_drag_classes(&strip_end);
    });

    overlay.add_controller(drag_source);

    // DropTarget: accepts reorder drops
    let drop_target = DropTarget::new(String::static_type(), gtk4::gdk::DragAction::MOVE);
    drop_target.set_actions(gtk4::gdk::DragAction::MOVE);

    let overlay_enter = overlay.clone();
    let strip_enter = strip.clone();
    drop_target.connect_enter(move |_, _, _| {
        clear_drag_target_class(&strip_enter);
        overlay_enter.add_css_class("drag-target");
        gtk4::gdk::DragAction::MOVE
    });

    let overlay_motion = overlay.clone();
    let strip_motion = strip.clone();
    drop_target.connect_motion(move |_, _, _| {
        clear_drag_target_class(&strip_motion);
        overlay_motion.add_css_class("drag-target");
        gtk4::gdk::DragAction::MOVE
    });

    let overlay_leave = overlay.clone();
    drop_target.connect_leave(move |_| {
        overlay_leave.remove_css_class("drag-target");
    });

    // connect_accept handles the actual drop logic
    let target_did = did;
    let p_apps = pinned_apps_ref.clone();
    let src_id_drop = drag_source_id.clone();
    let strip_drop = strip.clone();
    drop_target.connect_accept(move |dt, _| {
        let source_id = src_id_drop.borrow().clone();

        let Some(source_desktop_id) = source_id else {
            return false;
        };

        if source_desktop_id == target_did {
            return false;
        }

        // Find and reorder in data
        let mut pinned = p_apps.borrow_mut();
        let source_idx = pinned.iter().position(|d| *d == source_desktop_id);
        let target_idx = pinned.iter().position(|d| *d == target_did);

        let (Some(s), Some(t)) = (source_idx, target_idx) else {
            return false;
        };

        let item = pinned.remove(s);
        let insert_pos = t;
        pinned.insert(insert_pos, item);
        info!("Reordered Favorites: moved {source_desktop_id} to position {insert_pos}");
        drop(pinned);

        // Move source overlay widget to new position in strip
        let children = strip_drop.observe_children();
        if let Some(source_obj) = children.item(s as u32)
            && let Ok(source_overlay) = source_obj.downcast::<gtk4::Widget>()
        {
            strip_drop.remove(&source_overlay);
            if insert_pos == 0 {
                strip_drop.prepend(&source_overlay);
            } else if let Some(prev_obj) = children.item((insert_pos - 1) as u32)
                && let Ok(prev_widget) = prev_obj.downcast::<gtk4::Widget>()
            {
                strip_drop.insert_child_after(&source_overlay, Some(&prev_widget));
            } else {
                strip_drop.append(&source_overlay);
            }
        }

        // Persist to config
        save_pinned_apps(&p_apps.borrow());

        info!("Favorites reordered successfully");
        let _ = dt;
        true
    });

    overlay.add_controller(drop_target);
}

/// Update the pinned strip buttons based on current config and loaded apps
pub fn update_pinned_strip(
    strip: &GtkBox,
    pinned_apps: &[String],
    loaded_apps: &[DesktopApp],
    window: &libadwaita::ApplicationWindow,
    pinned_apps_ref: &Rc<RefCell<Vec<String>>>,
    _all_apps_ref: &Rc<RefCell<Vec<DesktopApp>>>,
    dragging: &Rc<Cell<bool>>,
) {
    while let Some(child) = strip.first_child() {
        strip.remove(&child);
    }

    if pinned_apps.is_empty() {
        return;
    }

    let drag_source_id: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

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

            // Overlay with remove badge (appears on hover)
            let overlay = Overlay::new();
            overlay.add_css_class("pinned-overlay");
            overlay.set_child(Some(&btn));

            let remove_badge = build_remove_badge();
            overlay.add_overlay(&remove_badge);

            // Left-click: launch app and hide window
            let exec = app.exec.clone();
            let terminal = app.terminal;
            let win_click = window.clone();
            btn.connect_clicked(move |_| {
                info!("Launching pinned app: {exec}");
                launch_app(&exec, terminal, None);
                win_click.hide();
            });

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
            let p_strip = strip.clone();
            let overlay_clone = overlay.clone();

            let badge_click = GestureClick::new();
            badge_click.set_button(1);
            badge_click.set_propagation_phase(gtk4::PropagationPhase::Capture);
            badge_click.connect_pressed(move |ctrl, _n_press, _, _| {
                ctrl.set_propagation_limit(gtk4::PropagationLimit::SameNative);
                {
                    let mut pinned = p_apps.borrow_mut();
                    pinned.retain(|d| d != &did);
                    info!("Removed from Favorites: {app_name}");
                }
                save_pinned_apps(&p_apps.borrow());
                p_strip.remove(&overlay_clone);
                update_strip_visibility(&p_strip, &p_apps.borrow(), true);
            });
            remove_badge.add_controller(badge_click);

            // Setup drag-and-drop reordering
            setup_drag_and_drop(
                &overlay,
                desktop_id,
                strip,
                pinned_apps_ref,
                &drag_source_id,
                dragging,
            );

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
    query_is_empty: bool,
    dragging: &Rc<Cell<bool>>,
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
        dragging,
    );
    update_strip_visibility(strip, &pinned, query_is_empty);
}
