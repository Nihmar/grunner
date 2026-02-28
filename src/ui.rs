use crate::actions::{
    launch_app, open_file_or_line, open_obsidian_file_line, open_obsidian_file_path, open_settings,
    perform_obsidian_action, power_action,
};
use crate::app_item::AppItem;
use crate::calc_item::CalcItem;
use crate::cmd_item::CommandItem;
use crate::config::Config;
use crate::launcher;
use crate::list_model::AppListModel;
use crate::obsidian_item::{ObsidianAction, ObsidianActionItem};
use crate::search_result_item::SearchResultItem;
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

// ---------------------------------------------------------------------------
// Async app-list loader
// ---------------------------------------------------------------------------

/// Polls for the result of the background `load_apps` thread and calls
/// `model.set_apps()` once it arrives. Uses the same idle-poll pattern as
/// `run_subprocess` in list_model to stay on the GTK main thread.
fn poll_apps(rx: std::sync::mpsc::Receiver<Vec<launcher::DesktopApp>>, model: AppListModel) {
    match rx.try_recv() {
        Ok(apps) => {
            model.set_apps(apps);
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            glib::idle_add_local_once(move || poll_apps(rx, model));
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extracts the argument following ":ob " from the search entry text.
/// Returns an empty string if none is present.
fn extract_obsidian_arg(text: &str) -> &str {
    text.strip_prefix(":ob ").map(str::trim).unwrap_or("")
}

/// Opens an Obsidian file from a grep output line of the form
/// `file_path:line_num:content`, falling back to just opening the file
/// if no line number is present.
fn open_obsidian_grep_line(line: &str, cfg: &crate::config::ObsidianConfig) {
    if let Some((file_path, rest)) = line.split_once(':') {
        if let Some((line_num, _)) = rest.split_once(':') {
            open_obsidian_file_line(file_path, line_num, cfg);
        } else {
            open_obsidian_file_path(file_path, cfg);
        }
    } else {
        open_obsidian_file_path(line, cfg);
    }
}

/// Activates the item at `obj`, performing the appropriate action based on its
/// type. Returns whether the window should be closed after activation.
fn activate_item(obj: &glib::Object, model: &AppListModel) {
    if let Some(app_item) = obj.downcast_ref::<AppItem>() {
        launch_app(&app_item.exec(), app_item.terminal());
    } else if let Some(calc_item) = obj.downcast_ref::<CalcItem>() {
        let result = calc_item.result();
        let number = result.strip_prefix("= ").unwrap_or(&result);
        let display = gtk4::gdk::Display::default().expect("cannot get display");
        display.clipboard().set_text(number);
    } else if let Some(cmd_item) = obj.downcast_ref::<CommandItem>() {
        if model.obsidian_grep_mode() {
            if let Some(cfg) = &model.obsidian_cfg {
                open_obsidian_grep_line(&cmd_item.line(), cfg);
            }
        } else if model.obsidian_file_mode() {
            if let Some(cfg) = &model.obsidian_cfg {
                open_obsidian_file_path(&cmd_item.line(), cfg);
            }
        } else {
            open_file_or_line(&cmd_item.line());
        }
    } else if let Some(obs_item) = obj.downcast_ref::<ObsidianActionItem>() {
        if let Some(cfg) = &model.obsidian_cfg {
            perform_obsidian_action(obs_item.action(), obs_item.arg().as_deref(), cfg);
        }
    } else if let Some(sr_item) = obj.downcast_ref::<SearchResultItem>() {
        let bus = sr_item.bus_name();
        let path = sr_item.object_path();
        let id = sr_item.id();
        let terms = sr_item.terms();
        std::thread::spawn(move || {
            crate::search_provider::activate_result(&bus, &path, &id, &terms);
        });
    }
}

/// Creates a styled icon+label button for the power bar.
fn make_icon_button(label: &str, icon_candidates: &[&str], icon_theme: &gtk4::IconTheme) -> Button {
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
    btn
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

pub fn build_ui(app: &Application, cfg: &Config) {
    // Obtain the display once and reuse throughout.
    let display = gtk4::gdk::Display::default().expect("Cannot connect to display");

    // Load CSS
    let provider = CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Model starts empty; apps are loaded in a background thread below.
    let model = AppListModel::new(
        cfg.max_results,
        cfg.calculator,
        cfg.commands.clone(),
        cfg.obsidian.clone(),
        cfg.command_debounce_ms,
        cfg.search_provider_blacklist.clone(),
    );

    let window = ApplicationWindow::builder()
        .application(app)
        .title("grunner")
        .default_width(cfg.window_width)
        .default_height(cfg.window_height)
        .decorated(false)
        .resizable(false)
        .build();
    window.set_css_classes(&[&"launcher-window"]);
    window.remove_css_class("background");
    window.connect_realize(|w| {
        w.remove_css_class("background");
    });

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("launcher-box");
    root.set_overflow(gtk4::Overflow::Hidden);

    // --- Search bar ---
    let entry_box = GtkBox::new(Orientation::Horizontal, 6);
    entry_box.set_hexpand(true);
    entry_box.set_margin_start(12);
    entry_box.set_margin_end(12);
    entry_box.set_margin_top(12);
    entry_box.set_margin_bottom(0);

    let command_icon = Image::new();
    command_icon.set_pixel_size(24);
    command_icon.set_valign(Align::Center);
    command_icon.set_visible(false);
    entry_box.append(&command_icon);

    let entry = Entry::builder()
        .placeholder_text("Search applications…")
        .hexpand(true)
        .build();
    entry.add_css_class("search-entry");
    entry_box.append(&entry);

    root.append(&entry_box);

    // --- Obsidian action button bar ---
    let obsidian_bar = GtkBox::new(Orientation::Horizontal, 8);
    obsidian_bar.set_halign(Align::Center);
    obsidian_bar.set_margin_top(6);
    obsidian_bar.set_margin_bottom(6);
    obsidian_bar.set_visible(false);

    let obsidian_actions = [
        ("Open Vault", ObsidianAction::OpenVault),
        ("New Note", ObsidianAction::NewNote),
        ("Daily Note", ObsidianAction::DailyNote),
        ("Quick Note", ObsidianAction::QuickNote),
    ];

    for (label, action) in obsidian_actions {
        let btn = Button::with_label(label);
        btn.add_css_class("power-button");

        btn.connect_clicked(clone!(
            #[strong]
            model,
            #[weak]
            window,
            #[weak]
            entry,
            move |_| {
                let current_text = entry.text();
                let arg = extract_obsidian_arg(&current_text);
                let arg_opt = if arg.is_empty() { None } else { Some(arg) };

                if let Some(cfg) = &model.obsidian_cfg {
                    perform_obsidian_action(action, arg_opt, cfg);
                }
                window.close();
            }
        ));
        obsidian_bar.append(&btn);
    }

    let power_bar = build_power_bar(&window, &entry);

    let factory = model.create_factory();
    let list_view = ListView::new(Some(model.selection.clone()), Some(factory));
    list_view.set_single_click_activate(false);
    list_view.add_css_class("app-list");
    list_view.set_can_focus(false);

    let scrolled = ScrolledWindow::builder()
        .vexpand(true)
        .child(&list_view)
        .build();

    root.append(&scrolled);
    root.append(&obsidian_bar);
    root.append(&power_bar);
    window.set_content(Some(&root));

    // Present only after the full widget tree is in place.
    window.present();

    // --- Window show handler: reset all state ---
    window.connect_show(clone!(
        #[weak]
        entry,
        #[weak]
        obsidian_bar,
        #[weak]
        command_icon,
        #[strong]
        model,
        move |_| {
            entry.set_text("");
            model.populate("");
            obsidian_bar.set_visible(false);
            command_icon.set_visible(false);
            entry.grab_focus();
        }
    ));

    // Resolve the Obsidian icon name once — reused in connect_changed below.
    let icon_theme = gtk4::IconTheme::for_display(&display);
    let obsidian_icon_name = ["obsidian", "md.obsidian.Obsidian", "text-x-markdown"]
        .iter()
        .find(|&&name| icon_theme.has_icon(name))
        .copied()
        .unwrap_or("text-x-markdown");

    // --- Entry changed handler ---
    entry.connect_changed(clone!(
        #[strong]
        model,
        #[weak]
        obsidian_bar,
        #[weak]
        command_icon,
        move |e| {
            let text = e.text().to_lowercase();
            model.populate(&text);
            obsidian_bar.set_visible(model.obsidian_action_mode() || model.obsidian_file_mode());

            // Update command icon based on active prefix
            if text.starts_with(":f") || text.starts_with(":fg") {
                command_icon.set_icon_name(Some("text-x-generic"));
                command_icon.set_visible(true);
            } else if text.starts_with(":s") {
                command_icon.set_icon_name(Some("system-search"));
                command_icon.set_visible(true);
            } else if text.starts_with(":ob") || text.starts_with(":obg") {
                command_icon.set_icon_name(Some(obsidian_icon_name));
                command_icon.set_visible(true);
            } else {
                command_icon.set_visible(false);
            }
        }
    ));

    // --- Keyboard navigation + activation ---
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
                        activate_item(&obj, &model);
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

    // --- List-view click activation ---
    list_view.connect_activate(clone!(
        #[weak]
        window,
        #[strong]
        model,
        move |_, pos| {
            if let Some(obj) = model.store.item(pos) {
                activate_item(&obj, &model);
            }
            window.close();
        }
    ));

    // Kick off background app loading. The window is already visible and
    // interactive at this point. When the thread finishes, poll_apps() calls
    // model.set_apps() on the main thread, which re-runs the current query
    // (empty on first open, or whatever the user has already typed).
    let dirs = cfg.app_dirs.clone();
    let model_poll = model.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(launcher::load_apps(&dirs));
    });
    glib::idle_add_local_once(move || poll_apps(rx, model_poll));
}

// ---------------------------------------------------------------------------
// Power bar
// ---------------------------------------------------------------------------

fn build_power_bar(window: &ApplicationWindow, entry: &Entry) -> GtkBox {
    let power_bar = GtkBox::new(Orientation::Horizontal, 8);
    power_bar.add_css_class("power-bar");
    power_bar.set_hexpand(true);
    power_bar.set_margin_top(4);
    power_bar.set_margin_bottom(8);
    power_bar.set_margin_start(12);
    power_bar.set_margin_end(12);

    let display = gtk4::gdk::Display::default().expect("Cannot connect to display");
    let icon_theme = gtk4::IconTheme::for_display(&display);

    // Settings button
    {
        let btn = make_icon_button(
            "Settings",
            &["preferences-system", "emblem-system", "settings-configure"],
            &icon_theme,
        );
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
        let btn = make_icon_button(label, icon_candidates, &icon_theme);

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
