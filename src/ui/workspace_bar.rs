//! Workspace window bar for Grunner.
//!
//! Renders a horizontal strip of buttons — one per open window on the current
//! GNOME workspace — placed between the search entry and the results list.
//!
//! Requires the **window-calls** GNOME Shell extension:
//! <https://extensions.gnome.org/extension/4724/window-calls/>
//!
//! The bar auto-refreshes every time the Grunner launcher window becomes visible.

use crate::actions::workspace::{self as ws, WindowInfo};
use crate::core::global_state::get_tokio_runtime;
use glib::clone;
use gtk4::{
    Box as GtkBox, Button, EventControllerMotion, EventControllerScroll,
    EventControllerScrollFlags, Image, Label, Orientation, Overlay, PolicyType, PropagationPhase,
    ScrolledWindow, gdk, prelude::*,
};
use libadwaita::ApplicationWindow;
use std::rc::Rc;

const MAX_TITLE_CHARS: usize = 22;

fn truncate(s: &str, max: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn resolve_icon(preferred: &str, theme: &gtk4::IconTheme) -> String {
    if theme.has_icon(preferred) {
        return preferred.to_owned();
    }

    let replacements = [
        ("org.gnome.", "gnome-"),
        ("org.freedesktop.", ""),
        ("com.", ""),
        ("net.", ""),
    ];
    for (prefix, replacement) in &replacements {
        if let Some(stripped) = preferred.strip_prefix(prefix) {
            let candidate = format!("{replacement}{stripped}");
            if theme.has_icon(&candidate) {
                return candidate;
            }
        }
    }

    if let Some(last) = preferred.rsplit('.').next()
        && theme.has_icon(last)
    {
        return last.to_owned();
    }

    if let Some(last) = preferred.rsplit('.').next() {
        let candidate = format!("gnome-{last}");
        if theme.has_icon(&candidate) {
            return candidate;
        }
    }

    "application-x-executable".to_owned()
}

fn build_close_badge() -> Button {
    let badge = Button::builder()
        .icon_name("window-close-symbolic")
        .halign(gtk4::Align::End)
        .valign(gtk4::Align::Start)
        .build();
    badge.add_css_class("workspace-close-badge");
    badge.set_visible(false);
    badge
}

fn populate(
    buttons_box: &GtkBox,
    scroll: &ScrolledWindow,
    windows: Vec<WindowInfo>,
    icon_theme: &gtk4::IconTheme,
    app_window: &ApplicationWindow,
    on_change: &Rc<dyn Fn()>,
) {
    log::debug!(
        "[workspace_bar] populate called with {} window(s), scroll visible={}",
        windows.len(),
        scroll.is_visible()
    );

    while let Some(child) = buttons_box.first_child() {
        buttons_box.remove(&child);
    }

    if windows.is_empty() {
        log::debug!("[workspace_bar] no windows, hiding bar");
        scroll.set_visible(false);
        return;
    }

    let window_count = windows.len();
    let all_ids: Vec<u64> = windows.iter().map(|w| w.id).collect();

    for info in &windows {
        log::debug!(
            "[workspace_bar] creating button id={} title={:?} icon={:?}",
            info.id,
            info.title,
            info.icon_name
        );

        let btn = Button::new();
        btn.add_css_class("workspace-window-btn");
        btn.set_tooltip_text(Some(&info.title));

        let inner = GtkBox::new(Orientation::Horizontal, 4);
        inner.add_css_class("workspace-window-btn-inner");

        let resolved_icon = resolve_icon(&info.icon_name, icon_theme);
        log::debug!(
            "[workspace_bar] resolved icon {:?} → {:?}",
            info.icon_name,
            resolved_icon
        );
        let icon = Image::from_icon_name(&resolved_icon);
        icon.add_css_class("workspace-window-icon");
        inner.append(&icon);

        let label = Label::new(Some(&truncate(&info.title, MAX_TITLE_CHARS)));
        label.add_css_class("workspace-window-label");
        inner.append(&label);

        btn.set_child(Some(&inner));

        let win_id = info.id;
        btn.connect_clicked(clone!(
            #[weak]
            app_window,
            move |_| {
                glib::spawn_future_local(async move {
                    ws::activate_window(win_id).await;
                });
                app_window.hide();
            }
        ));

        let overlay = Overlay::new();
        overlay.set_child(Some(&btn));

        let close_badge = build_close_badge();
        overlay.add_overlay(&close_badge);

        let motion = EventControllerMotion::new();
        motion.connect_enter(clone!(
            #[weak]
            close_badge,
            move |_, _, _| {
                close_badge.set_visible(true);
            }
        ));
        motion.connect_leave(clone!(
            #[weak]
            close_badge,
            move |_| {
                close_badge.set_visible(false);
            }
        ));
        overlay.add_controller(motion);

        let badge_win_id = info.id;
        let refresh_badge = on_change.clone();
        close_badge.connect_clicked(move |_| {
            let refresh = refresh_badge.clone();
            glib::spawn_future_local(async move {
                ws::close_window(badge_win_id).await;
                refresh();
            });
        });

        buttons_box.append(&overlay);
    }

    let separator = gtk4::Separator::new(Orientation::Horizontal);
    separator.add_css_class("workspace-separator-h");
    buttons_box.append(&separator);

    let close_all_btn = Button::builder()
        .icon_name("window-close-symbolic")
        .halign(gtk4::Align::Center)
        .build();
    close_all_btn.add_css_class("workspace-close-all-btn");
    close_all_btn.set_tooltip_text(Some("Close all windows"));

    let ids_for_close_all = all_ids.clone();
    let refresh_close_all = on_change.clone();
    close_all_btn.connect_clicked(move |_| {
        let ids = ids_for_close_all.clone();
        let refresh = refresh_close_all.clone();
        glib::spawn_future_local(async move {
            ws::close_all_windows(ids).await;
            refresh();
        });
    });
    buttons_box.append(&close_all_btn);

    log::debug!("[workspace_bar] populate done, showing scroll");
    scroll.set_visible(true);

    if window_count > 6 {
        scroll.add_css_class("tall");
        buttons_box.set_margin_bottom(12);
    } else {
        scroll.remove_css_class("tall");
        buttons_box.set_margin_bottom(0);
    }
}

fn spawn_refresh(
    scroll: &ScrolledWindow,
    buttons_box: &GtkBox,
    window: &ApplicationWindow,
    on_change: &Rc<dyn Fn()>,
) {
    spawn_refresh_delayed(scroll, buttons_box, window, on_change, 0);
}

fn spawn_refresh_delayed(
    scroll: &ScrolledWindow,
    buttons_box: &GtkBox,
    window: &ApplicationWindow,
    on_change: &Rc<dyn Fn()>,
    delay_ms: u64,
) {
    let (tx, rx) = std::sync::mpsc::channel::<Option<Vec<WindowInfo>>>();

    std::thread::spawn(move || {
        if delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
        let rt = get_tokio_runtime();
        let windows = rt.block_on(ws::fetch_workspace_windows());
        log::debug!(
            "[workspace_bar] background thread result: {:?}",
            windows.as_ref().map(std::vec::Vec::len)
        );
        let _ = tx.send(windows);
    });

    let oc = on_change.clone();
    glib::idle_add_local_once(clone!(
        #[weak]
        scroll,
        #[weak]
        buttons_box,
        #[weak]
        window,
        move || {
            poll_windows(rx, scroll, buttons_box, window, oc);
        }
    ));
}

#[must_use]
pub fn build_workspace_bar(window: &ApplicationWindow) -> ScrolledWindow {
    let scroll = ScrolledWindow::builder()
        .vscrollbar_policy(PolicyType::Automatic)
        .hscrollbar_policy(PolicyType::Never)
        .min_content_height(1)
        .build();
    scroll.add_css_class("workspace-bar");
    scroll.set_visible(false);

    let buttons_box = GtkBox::new(Orientation::Vertical, 6);
    buttons_box.add_css_class("workspace-bar-buttons");
    scroll.set_child(Some(&buttons_box));

    let scroll_controller = EventControllerScroll::new(EventControllerScrollFlags::BOTH_AXES);
    scroll_controller.set_propagation_phase(PropagationPhase::Capture);

    let scroll_clone = scroll.clone();
    scroll_controller.connect_scroll(move |_, dx, dy| {
        log::debug!("[workspace_bar] scroll event: dx={dx}, dy={dy}");
        let adjustment = scroll_clone.hadjustment();
        let delta = if dx == 0.0 { dy } else { dx };
        let new_value = adjustment.value() + delta * adjustment.step_increment();
        adjustment.set_value(new_value.clamp(0.0, adjustment.upper() - adjustment.page_size()));
        glib::Propagation::Stop
    });

    scroll.add_controller(scroll_controller);

    let on_change_cell: Rc<std::cell::RefCell<Option<Rc<dyn Fn()>>>> =
        Rc::new(std::cell::RefCell::new(None));
    let oc_cell = on_change_cell.clone();
    let scroll_r = scroll.clone();
    let buttons_r = buttons_box.clone();
    let window_r = window.clone();
    let on_change: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(ref cb) = *oc_cell.borrow() {
            spawn_refresh_delayed(&scroll_r, &buttons_r, &window_r, cb, 350);
        }
    });
    on_change_cell.borrow_mut().replace(on_change.clone());
    let oc = on_change.clone();

    window.connect_map(clone!(
        #[weak]
        scroll,
        #[weak]
        buttons_box,
        #[weak]
        window,
        move |_| {
            log::debug!("[workspace_bar] connect_map fired, launching fetch thread");
            spawn_refresh(&scroll, &buttons_box, &window, &oc);
        }
    ));

    scroll
}

fn poll_windows(
    rx: std::sync::mpsc::Receiver<Option<Vec<WindowInfo>>>,
    scroll: ScrolledWindow,
    buttons_box: GtkBox,
    window: ApplicationWindow,
    on_change: Rc<dyn Fn()>,
) {
    match rx.try_recv() {
        Ok(Some(windows)) => {
            log::debug!(
                "[workspace_bar] poll_windows received {} window(s)",
                windows.len()
            );
            let Some(display) = gdk::Display::default() else {
                return;
            };
            let icon_theme = gtk4::IconTheme::for_display(&display);
            populate(
                &buttons_box,
                &scroll,
                windows,
                &icon_theme,
                &window,
                &on_change,
            );
        }
        Ok(None) => {
            log::debug!("[workspace_bar] extension not available, hiding bar");
            scroll.set_visible(false);
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            glib::idle_add_local_once(move || {
                poll_windows(rx, scroll, buttons_box, window, on_change)
            });
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            log::warn!("[workspace_bar] background thread disconnected without sending");
        }
    }
}
