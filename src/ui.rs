use crate::actions::{
    launch_app, open_file_or_line, open_obsidian_file_line, open_obsidian_file_path,
    perform_obsidian_action,
};
use crate::app_item::AppItem;
use crate::app_mode::AppMode;
use crate::calc_item::CalcItem;
use crate::cmd_item::CommandItem;
use crate::config::Config;
use crate::launcher;
use crate::list_model::AppListModel;
use crate::obsidian_bar::build_obsidian_bar;
use crate::obsidian_item::ObsidianActionItem;
use crate::power_bar::build_power_bar;
use crate::search_result_item::SearchResultItem;
use glib::clone;
use gtk4::gdk::Key;
use gtk4::prelude::DisplayExt;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, CssProvider, Entry, EventControllerKey, Image, ListView, Orientation,
    ScrolledWindow,
};
use libadwaita::prelude::AdwApplicationWindowExt;
use libadwaita::{Application, ApplicationWindow};
use std::cell::Cell;
use std::rc::Rc;

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
/// type and the current `AppMode`. Returns whether the window should be closed
/// after activation.
fn activate_item(obj: &glib::Object, model: &AppListModel, mode: AppMode) {
    if let Some(app_item) = obj.downcast_ref::<AppItem>() {
        launch_app(&app_item.exec(), app_item.terminal());
    } else if let Some(calc_item) = obj.downcast_ref::<CalcItem>() {
        let result = calc_item.result();
        let number = result.strip_prefix("= ").unwrap_or(&result);
        let display = gtk4::gdk::Display::default().expect("cannot get display");
        display.clipboard().set_text(number);
    } else if let Some(cmd_item) = obj.downcast_ref::<CommandItem>() {
        let line = cmd_item.line();
        match mode {
            AppMode::ObsidianGrep => {
                if let Some(cfg) = &model.obsidian_cfg {
                    open_obsidian_grep_line(&line, cfg);
                }
            }
            AppMode::Obsidian | AppMode::FileSearch => {
                if let Some(cfg) = &model.obsidian_cfg {
                    open_obsidian_file_path(&line, cfg);
                }
            }
            _ => {
                open_file_or_line(&line);
            }
        }
    } else if let Some(obs_item) = obj.downcast_ref::<ObsidianActionItem>() {
        if let Some(cfg) = &model.obsidian_cfg {
            perform_obsidian_action(obs_item.action(), obs_item.arg().as_deref(), cfg);
        }
    } else if let Some(sr_item) = obj.downcast_ref::<SearchResultItem>() {
        let (bus, path, id, terms) = (
            sr_item.bus_name(),
            sr_item.object_path(),
            sr_item.id(),
            sr_item.terms(),
        );
        std::thread::spawn(move || {
            crate::search_provider::activate_result(&bus, &path, &id, &terms);
        });
    }
}

/// Moves the list selection to `pos` and scrolls it into view.
fn scroll_selection_to(model: &AppListModel, list_view: &ListView, pos: u32) {
    model.selection.set_selected(pos);
    let _ = list_view.activate_action("list.scroll-to-item", Some(&pos.to_variant()));
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

    // Shared, single source of truth for the current typing mode.
    // Wrapped in Rc<Cell> so it can be captured by multiple closures on the
    // GTK main thread without needing a Mutex.
    let current_mode: Rc<Cell<AppMode>> = Rc::new(Cell::new(AppMode::Normal));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("grunner")
        .default_width(cfg.window_width)
        .default_height(cfg.window_height)
        .decorated(false)
        .resizable(false)
        .build();
    // Remove the "background" class only after realization — the pre-realize
    // call would be a no-op since the style context isn't live yet.
    window.set_css_classes(&["launcher-window"]);
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

    // --- Bars ---
    let obsidian_bar = build_obsidian_bar(&window, &entry, &model);
    let icon_theme = gtk4::IconTheme::for_display(&display);
    let power_bar = build_power_bar(&window, &entry, &icon_theme);

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
        #[strong]
        current_mode,
        move |_| {
            entry.set_text("");
            model.populate("");
            current_mode.set(AppMode::Normal);
            obsidian_bar.set_visible(false);
            command_icon.set_visible(false);
            entry.grab_focus();
        }
    ));

    // Resolve the Obsidian icon name once — reused in connect_changed below.
    let obsidian_icon_name = ["obsidian", "md.obsidian.Obsidian", "text-x-markdown"]
        .iter()
        .find(|&&name| icon_theme.has_icon(name))
        .copied()
        .unwrap_or("text-x-markdown");

    // --- Entry changed handler ---
    // The mode is derived once from the text here and stored in `current_mode`
    // so the key handler and click handler can read it without re-parsing.
    entry.connect_changed(clone!(
        #[strong]
        model,
        #[strong]
        current_mode,
        #[weak]
        obsidian_bar,
        #[weak]
        command_icon,
        move |e| {
            let text = e.text().to_lowercase();
            let mode = AppMode::from_text(&text);
            current_mode.set(mode);

            model.populate(&text);

            obsidian_bar.set_visible(mode.show_obsidian_bar());

            match mode.icon_name(obsidian_icon_name) {
                Some(name) => {
                    command_icon.set_icon_name(Some(name));
                    command_icon.set_visible(true);
                }
                None => command_icon.set_visible(false),
            }

            // Force a full repaint to avoid stale pixel artifacts left behind
            // when text is deleted quickly (GTK4 doesn't always invalidate the
            // full previously-painted area on its own).
            e.queue_draw();
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
        #[strong]
        current_mode,
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
                        activate_item(&obj, &model, current_mode.get());
                    }
                    window.close();
                    glib::Propagation::Stop
                }
                Key::Down | Key::KP_Down => {
                    let pos = model.selection.selected();
                    let n = model.store.n_items();
                    if pos + 1 < n {
                        scroll_selection_to(&model, &list_view, pos + 1);
                    }
                    glib::Propagation::Stop
                }
                Key::Up | Key::KP_Up => {
                    let pos = model.selection.selected();
                    if pos > 0 {
                        scroll_selection_to(&model, &list_view, pos - 1);
                    }
                    glib::Propagation::Stop
                }
                Key::Page_Down => {
                    let pos = model.selection.selected();
                    let n = model.store.n_items();
                    let next = (pos + 10).min(n.saturating_sub(1));
                    scroll_selection_to(&model, &list_view, next);
                    glib::Propagation::Stop
                }
                Key::Page_Up => {
                    let pos = model.selection.selected();
                    scroll_selection_to(&model, &list_view, pos.saturating_sub(10));
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
        #[strong]
        current_mode,
        move |_, pos| {
            if let Some(obj) = model.store.item(pos) {
                activate_item(&obj, &model, current_mode.get());
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
