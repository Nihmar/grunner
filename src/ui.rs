use crate::actions::{
    launch_app, open_file_or_line, open_settings, perform_obsidian_action, power_action,
};
use crate::app_item::AppItem;
use crate::calc_item::CalcItem;
use crate::cmd_item::CommandItem;
use crate::config::Config;
use crate::launcher;
use crate::list_model::AppListModel;
use crate::obsidian_item::ObsidianActionItem;
use glib::clone;
use gtk4::gdk::Key;
use gtk4::prelude::DisplayExt;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CssProvider, Entry, EventControllerKey, Image, Label, ListView,
    Orientation, ScrolledWindow,
};
use libadwaita::prelude::{AdwApplicationWindowExt, AdwDialogExt, AlertDialogExt};
use libadwaita::{AlertDialog, Application, ApplicationWindow, ResponseAppearance};
use std::rc::Rc; // <-- new

pub fn build_ui(app: &Application, cfg: &Config) {
    // Load CSS
    let provider = CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Cannot connect to display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let all_apps: Rc<Vec<launcher::DesktopApp>> = Rc::new(launcher::load_apps(&cfg.app_dirs));
    let obsidian_cfg = cfg.obsidian.clone(); // <-- new

    let model = AppListModel::new(
        all_apps,
        cfg.max_results,
        cfg.calculator,
        cfg.commands.clone(),
        obsidian_cfg, // <-- pass it to the model
    );

    let window = ApplicationWindow::builder()
        .application(app)
        .title("grunner")
        .default_width(cfg.window_width)
        .default_height(cfg.window_height)
        .decorated(false)
        .resizable(false)
        .build();
    window.set_css_classes(&["launcher-window"]);
    window.remove_css_class("background");
    window.connect_realize(|w| {
        w.remove_css_class("background");
    });

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("launcher-box");
    root.set_overflow(gtk4::Overflow::Hidden);

    let entry = Entry::builder()
        .placeholder_text("Search applications…")
        .hexpand(true)
        .build();
    entry.add_css_class("search-entry");

    let power_bar = build_power_bar(&window, &entry);

    let factory = AppListModel::create_factory();
    let list_view = ListView::new(Some(model.selection.clone()), Some(factory));
    list_view.set_single_click_activate(false);
    list_view.add_css_class("app-list");

    let scrolled = ScrolledWindow::builder()
        .vexpand(true)
        .child(&list_view)
        .build();

    root.append(&entry);
    root.append(&scrolled);
    root.append(&power_bar);
    window.set_content(Some(&root));

    window.connect_show(clone!(
        #[weak]
        entry,
        #[strong]
        model,
        move |_| {
            entry.set_text("");
            model.populate("");
            entry.grab_focus();
        }
    ));

    entry.connect_changed(clone!(
        #[strong]
        model,
        move |e| {
            model.populate(&e.text());
        }
    ));

    let key_ctrl = EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    key_ctrl.connect_key_pressed(clone!(
        #[weak]
        list_view,
        #[weak]
        window,
        #[strong]
        model,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            match key {
                Key::Escape => {
                    window.close();
                    glib::Propagation::Stop
                }
                Key::Return | Key::KP_Enter => {
                    let pos = model.selection.selected();
                    if let Some(obj) = model.store.item(pos) {
                        if let Some(app_item) = obj.downcast_ref::<AppItem>() {
                            launch_app(&app_item.exec(), app_item.terminal());
                        } else if let Some(calc_item) = obj.downcast_ref::<CalcItem>() {
                            let result = calc_item.result();
                            let number = result.strip_prefix("= ").unwrap_or(&result);
                            let display =
                                gtk4::gdk::Display::default().expect("cannot get display");
                            let clipboard = display.clipboard();
                            clipboard.set_text(number);
                        } else if let Some(cmd_item) = obj.downcast_ref::<CommandItem>() {
                            open_file_or_line(&cmd_item.line());
                        } else if let Some(obs_item) = obj.downcast_ref::<ObsidianActionItem>() {
                            // <-- new branch
                            if let Some(cfg) = &model.obsidian_cfg {
                                let action = obs_item.action();
                                let arg = obs_item.arg();
                                perform_obsidian_action(action, arg.as_deref(), cfg);
                            }
                        }
                    }
                    window.close();
                    glib::Propagation::Stop
                }
                Key::Down | Key::KP_Down => {
                    let pos = model.selection.selected();
                    let n = model.store.n_items();
                    if pos + 1 < n {
                        let next = pos + 1;
                        model.selection.set_selected(next);
                        let _ = list_view
                            .activate_action("list.scroll-to-item", Some(&next.to_variant()));
                    }
                    glib::Propagation::Stop
                }
                Key::Up | Key::KP_Up => {
                    let pos = model.selection.selected();
                    if pos > 0 {
                        let prev = pos - 1;
                        model.selection.set_selected(prev);
                        let _ = list_view
                            .activate_action("list.scroll-to-item", Some(&prev.to_variant()));
                    }
                    glib::Propagation::Stop
                }
                Key::Page_Down => {
                    let pos = model.selection.selected();
                    let n = model.store.n_items();
                    let next = (pos + 10).min(n.saturating_sub(1));
                    model.selection.set_selected(next);
                    let _ =
                        list_view.activate_action("list.scroll-to-item", Some(&next.to_variant()));
                    glib::Propagation::Stop
                }
                Key::Page_Up => {
                    let pos = model.selection.selected();
                    let prev = pos.saturating_sub(10);
                    model.selection.set_selected(prev);
                    let _ =
                        list_view.activate_action("list.scroll-to-item", Some(&prev.to_variant()));
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        }
    ));
    entry.add_controller(key_ctrl);

    list_view.connect_activate(clone!(
        #[weak]
        window,
        #[strong]
        model,
        move |_, pos| {
            if let Some(obj) = model.store.item(pos) {
                if let Some(app_item) = obj.downcast_ref::<AppItem>() {
                    launch_app(&app_item.exec(), app_item.terminal());
                    window.close();
                } else if let Some(calc_item) = obj.downcast_ref::<CalcItem>() {
                    let result = calc_item.result();
                    let number = result.strip_prefix("= ").unwrap_or(&result);
                    let display = gtk4::gdk::Display::default().expect("cannot get display");
                    let clipboard = display.clipboard();
                    clipboard.set_text(number);
                    window.close();
                } else if let Some(cmd_item) = obj.downcast_ref::<CommandItem>() {
                    open_file_or_line(&cmd_item.line());
                    window.close();
                } else if let Some(obs_item) = obj.downcast_ref::<ObsidianActionItem>() {
                    // <-- new branch
                    if let Some(cfg) = &model.obsidian_cfg {
                        let action = obs_item.action();
                        let arg = obs_item.arg();
                        perform_obsidian_action(action, arg.as_deref(), cfg);
                    }
                    window.close();
                }
            } else {
                window.close();
            }
        }
    ));

    window.present();
    entry.grab_focus();
    model.populate("");
}

// build_power_bar remains unchanged
fn build_power_bar(window: &ApplicationWindow, entry: &Entry) -> GtkBox {
    let power_bar = GtkBox::new(Orientation::Horizontal, 8);
    power_bar.add_css_class("power-bar");
    power_bar.set_hexpand(true);
    power_bar.set_margin_top(4);
    power_bar.set_margin_bottom(8);
    power_bar.set_margin_start(12);
    power_bar.set_margin_end(12);

    let icon_theme = gtk4::IconTheme::for_display(
        &gtk4::gdk::Display::default().expect("Cannot connect to display"),
    );

    // Settings button
    {
        let btn = Button::new();
        btn.add_css_class("power-button");

        let btn_box = GtkBox::new(Orientation::Horizontal, 6);
        btn_box.set_halign(Align::Center);

        let settings_icon = ["preferences-system", "emblem-system", "settings-configure"]
            .iter()
            .find(|&&n| icon_theme.has_icon(n))
            .copied()
            .unwrap_or("preferences-system");
        let image = Image::from_icon_name(settings_icon);
        image.set_pixel_size(16);
        btn_box.append(&image);
        btn_box.append(&Label::new(Some("Settings")));
        btn.set_child(Some(&btn_box));

        btn.connect_clicked(clone!(
            #[weak]
            window,
            move |_| {
                open_settings();
                window.close();
            }
        ));
        power_bar.append(&btn);
    }

    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    power_bar.append(&spacer);

    for (label, icon_candidates, action) in [
        (
            "Suspend",
            &[
                "system-suspend",
                "system-suspend-hibernate",
                "media-playback-pause",
            ][..],
            "suspend",
        ),
        (
            "Restart",
            &["system-restart", "system-reboot", "view-refresh"][..],
            "reboot",
        ),
        (
            "Power off",
            &["system-shutdown", "system-power-off"][..],
            "poweroff",
        ),
        (
            "Log out",
            &["system-log-out", "application-exit"][..],
            "logout",
        ),
    ] {
        let btn = Button::new();
        btn.add_css_class("power-button");

        let btn_box = GtkBox::new(Orientation::Horizontal, 6);
        btn_box.set_halign(Align::Center);

        if let Some(&icon_name) = icon_candidates.iter().find(|&&n| icon_theme.has_icon(n)) {
            let image = Image::from_icon_name(icon_name);
            image.set_pixel_size(16);
            btn_box.append(&image);
        }

        btn_box.append(&Label::new(Some(label)));
        btn.set_child(Some(&btn_box));

        let action = action.to_string();
        let label_str = label.to_string();
        btn.connect_clicked(clone!(
            #[weak]
            window,
            #[weak]
            entry,
            move |_| {
                let dialog = AlertDialog::builder()
                    .heading(format!("{}?", label_str))
                    .body(format!(
                        "Are you sure you want to {}?",
                        label_str.to_lowercase()
                    ))
                    .default_response("cancel")
                    .close_response("cancel")
                    .build();
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("confirm", &label_str);
                dialog.set_response_appearance("confirm", ResponseAppearance::Destructive);
                let action = action.clone();
                dialog.connect_response(
                    None,
                    clone!(
                        #[weak]
                        window,
                        #[weak]
                        entry,
                        move |_, response| {
                            if response == "confirm" {
                                window.close();
                                power_action(&action);
                            } else {
                                entry.grab_focus();
                            }
                        }
                    ),
                );
                dialog.present(Some(&window));
            }
        ));
        power_bar.append(&btn);
    }

    power_bar
}
