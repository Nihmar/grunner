use crate::actions::{
    launch_app, open_file_or_line, open_obsidian_file_line, open_obsidian_file_path,
    perform_obsidian_action,
};
use crate::app_item::AppItem;
use crate::app_mode::AppMode;

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

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, CssProvider, Entry, EventControllerKey, Image, ListView, Orientation,
    ScrolledWindow,
};
use libadwaita::prelude::AdwApplicationWindowExt;
use libadwaita::{Application, ApplicationWindow};
use std::cell::Cell;
use std::rc::Rc;








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




fn activate_item(obj: &glib::Object, model: &AppListModel, mode: AppMode) {
    if let Some(app_item) = obj.downcast_ref::<AppItem>() {
        launch_app(&app_item.exec(), app_item.terminal());





    } else if let Some(cmd_item) = obj.downcast_ref::<CommandItem>() {
        let line = cmd_item.line();
        match mode {
            AppMode::ObsidianGrep => {
                if let Some(cfg) = &model.obsidian_cfg {
                    open_obsidian_grep_line(&line, cfg);
                }
            }
            AppMode::Obsidian => {
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


fn scroll_selection_to(model: &AppListModel, list_view: &ListView, pos: u32) {
    model.selection.set_selected(pos);
    let _ = list_view.activate_action("list.scroll-to-item", Some(&pos.to_variant()));
}





pub fn build_ui(app: &Application, cfg: &Config) {

    let display = gtk4::gdk::Display::default().expect("Cannot connect to display");


    let provider = CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );


    let model = AppListModel::new(
        cfg.max_results,

        cfg.commands.clone(),
        cfg.obsidian.clone(),
        cfg.command_debounce_ms,
        cfg.search_provider_blacklist.clone(),
    );




    let current_mode: Rc<Cell<AppMode>> = Rc::new(Cell::new(AppMode::Normal));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("grunner")
        .default_width(cfg.window_width)
        .default_height(cfg.window_height)
        .decorated(false)
        .resizable(false)
        .build();


    window.set_css_classes(&["launcher-window"]);
    window.connect_realize(|w| {
        w.remove_css_class("background");
    });

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("launcher-box");
    root.set_overflow(gtk4::Overflow::Hidden);


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


    window.present();


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


    let obsidian_icon_name = ["obsidian", "md.obsidian.Obsidian", "text-x-markdown"]
        .iter()
        .find(|&&name| icon_theme.has_icon(name))
        .copied()
        .unwrap_or("text-x-markdown");




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




            e.queue_draw();
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





    let dirs = cfg.app_dirs.clone();
    let model_poll = model.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(launcher::load_apps(&dirs));
    });
    glib::idle_add_local_once(move || poll_apps(rx, model_poll));
}
