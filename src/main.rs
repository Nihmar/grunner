mod launcher;

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use glib::clone;
use glib::subclass::prelude::*;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CssProvider, Entry, EventControllerKey, Image, Label, ListItem,
    ListView, Orientation, ScrolledWindow, SignalListItemFactory, SingleSelection,
};
use launcher::DesktopApp;
use libadwaita::prelude::{AdwApplicationWindowExt, AdwDialogExt, AlertDialogExt};
use libadwaita::{AlertDialog, Application, ApplicationWindow, ResponseAppearance};
use std::rc::Rc;

const APP_ID: &str = "org.nihmar.grunner";
const WINDOW_WIDTH: i32 = 640;
const WINDOW_HEIGHT: i32 = 480;
const MAX_RESULTS: usize = 64;

// ── AppItem GObject ───────────────────────────────────────────────────────────
//
// A minimal GObject wrapper around DesktopApp so we can store it in a
// gio::ListStore and hand it to the ListView factory.

mod imp {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct AppItemInner {
        pub name: String,
        pub description: String,
        pub icon: String,
        pub exec: String,
        pub terminal: bool,
    }

    #[derive(Default)]
    pub struct AppItem {
        pub data: RefCell<AppItemInner>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppItem {
        const NAME: &'static str = "GrunnerAppItem";
        type Type = super::AppItem;
    }

    impl ObjectImpl for AppItem {}
}

glib::wrapper! {
    pub struct AppItem(ObjectSubclass<imp::AppItem>);
}

impl AppItem {
    pub fn new(app: &DesktopApp) -> Self {
        let obj: Self = glib::Object::new();
        *obj.imp().data.borrow_mut() = imp::AppItemInner {
            name: app.name.clone(),
            description: app.description.clone(),
            icon: app.icon.clone(),
            exec: app.exec.clone(),
            terminal: app.terminal,
        };
        obj
    }

    pub fn name(&self) -> String {
        self.imp().data.borrow().name.clone()
    }
    pub fn description(&self) -> String {
        self.imp().data.borrow().description.clone()
    }
    pub fn icon(&self) -> String {
        self.imp().data.borrow().icon.clone()
    }
    pub fn exec(&self) -> String {
        self.imp().data.borrow().exec.clone()
    }
    pub fn terminal(&self) -> bool {
        self.imp().data.borrow().terminal
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        build_ui(app);
    });
    app.run()
}

fn build_ui(app: &Application) {
    if let Some(window) = app.windows().first() {
        window.present();
        return;
    }

    let provider = CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Cannot connect to display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let all_apps: Rc<Vec<DesktopApp>> = Rc::new(launcher::load_apps());

    let window = ApplicationWindow::builder()
        .application(app)
        .title("grunner")
        .default_width(WINDOW_WIDTH)
        .default_height(WINDOW_HEIGHT)
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

    // ── Power bar ─────────────────────────────────────────────────────────────
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
        btn.set_hexpand(true);

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

    // ── Model ─────────────────────────────────────────────────────────────────
    let store = gio::ListStore::new::<AppItem>();
    let selection = SingleSelection::new(Some(store.clone()));
    selection.set_autoselect(true);
    selection.set_can_unselect(false);

    // ── Factory ───────────────────────────────────────────────────────────────
    //
    // setup:  called once per recycled row — build the widget tree
    // bind:   called when a row is about to be displayed — fill in data
    // unbind: called when a row is scrolled away — clear heavy refs

    let factory = SignalListItemFactory::new();

    factory.connect_setup(|_, list_item| {
        let list_item = list_item.downcast_ref::<ListItem>().unwrap();

        let hbox = GtkBox::new(Orientation::Horizontal, 12);
        hbox.set_margin_top(6);
        hbox.set_margin_bottom(6);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);

        let image = Image::new();
        image.set_pixel_size(32);
        image.set_valign(Align::Center);
        image.add_css_class("app-icon");
        hbox.append(&image);

        let vbox = GtkBox::new(Orientation::Vertical, 2);
        vbox.set_valign(Align::Center);
        vbox.set_hexpand(true);

        let name_label = Label::new(None);
        name_label.set_halign(Align::Start);
        name_label.add_css_class("row-name");
        vbox.append(&name_label);

        let desc_label = Label::new(None);
        desc_label.set_halign(Align::Start);
        desc_label.add_css_class("row-desc");
        desc_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        desc_label.set_max_width_chars(70);
        vbox.append(&desc_label);

        hbox.append(&vbox);
        list_item.set_child(Some(&hbox));
    });

    factory.connect_bind(|_, list_item| {
        let list_item = list_item.downcast_ref::<ListItem>().unwrap();
        let item = match list_item.item().and_then(|o| o.downcast::<AppItem>().ok()) {
            Some(i) => i,
            None => return,
        };
        let hbox = match list_item.child().and_then(|w| w.downcast::<GtkBox>().ok()) {
            Some(b) => b,
            None => return,
        };

        // Widget tree: hbox → [image, vbox → [name_label, desc_label]]
        let image = hbox
            .first_child()
            .and_then(|w| w.downcast::<Image>().ok())
            .unwrap();
        let vbox = hbox
            .last_child()
            .and_then(|w| w.downcast::<GtkBox>().ok())
            .unwrap();
        let name_label = vbox
            .first_child()
            .and_then(|w| w.downcast::<Label>().ok())
            .unwrap();
        let desc_label = vbox
            .last_child()
            .and_then(|w| w.downcast::<Label>().ok())
            .unwrap();

        // Set icon
        let icon = item.icon();
        if icon.is_empty() {
            image.set_icon_name(Some("application-x-executable"));
        } else if icon.starts_with('/') {
            image.set_from_file(Some(&icon));
        } else {
            image.set_icon_name(Some(&icon));
        }

        name_label.set_text(&item.name());

        let desc = item.description();
        if desc.is_empty() {
            desc_label.set_visible(false);
            desc_label.set_text("");
        } else {
            desc_label.set_visible(true);
            desc_label.set_text(&desc);
        }
    });

    factory.connect_unbind(|_, list_item| {
        let list_item = list_item.downcast_ref::<ListItem>().unwrap();
        if let Some(hbox) = list_item.child().and_then(|w| w.downcast::<GtkBox>().ok()) {
            if let Some(image) = hbox.first_child().and_then(|w| w.downcast::<Image>().ok()) {
                image.clear();
            }
        }
    });

    // ── ListView ──────────────────────────────────────────────────────────────
    let list_view = ListView::new(Some(selection.clone()), Some(factory));
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

    // ── Populate ──────────────────────────────────────────────────────────────
    let populate = {
        let store = store.clone();
        let selection = selection.clone();
        let all_apps = Rc::clone(&all_apps);

        Rc::new(move |query: &str| {
            store.remove_all();

            let matcher = SkimMatcherV2::default();

            let mut results: Vec<(i64, &DesktopApp)> = all_apps
                .iter()
                .filter_map(|app| {
                    if query.is_empty() {
                        Some((0i64, app))
                    } else {
                        let name_score = matcher.fuzzy_match(&app.name, query).unwrap_or(i64::MIN);
                        let desc_score = if !app.description.is_empty() {
                            matcher
                                .fuzzy_match(&app.description, query)
                                .unwrap_or(i64::MIN)
                                / 2
                        } else {
                            i64::MIN
                        };
                        let score = name_score.max(desc_score);
                        if score == i64::MIN {
                            None
                        } else {
                            Some((score, app))
                        }
                    }
                })
                .collect();

            if query.is_empty() {
                results.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
            } else {
                results.sort_by(|a, b| b.0.cmp(&a.0));
            }

            if !query.is_empty() {
                results.truncate(MAX_RESULTS);
            }

            let items: Vec<AppItem> = results.iter().map(|(_, app)| AppItem::new(app)).collect();
            store.extend_from_slice(&items);

            if store.n_items() > 0 {
                selection.set_selected(0);
            }
        })
    };

    // Reset state every time the window becomes visible
    window.connect_show(clone!(
        #[weak]
        entry,
        #[strong]
        populate,
        move |_| {
            entry.set_text("");
            populate("");
            entry.grab_focus();
        }
    ));

    // ── Search ────────────────────────────────────────────────────────────────
    entry.connect_changed(clone!(
        #[strong]
        populate,
        move |e| {
            populate(&e.text());
        }
    ));

    // ── Keyboard ──────────────────────────────────────────────────────────────
    let key_ctrl = EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    key_ctrl.connect_key_pressed(clone!(
        #[weak]
        list_view,
        #[weak]
        window,
        #[strong]
        store,
        #[strong]
        selection,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            use gtk4::gdk::Key;
            match key {
                Key::Escape => {
                    window.close();
                    glib::Propagation::Stop
                }
                Key::Return | Key::KP_Enter => {
                    let pos = selection.selected();
                    if let Some(item) = store.item(pos).and_then(|o| o.downcast::<AppItem>().ok()) {
                        launch_app(&item.exec(), item.terminal());
                    }
                    window.close();
                    glib::Propagation::Stop
                }
                Key::Down | Key::KP_Down => {
                    let pos = selection.selected();
                    let n = store.n_items();
                    if pos + 1 < n {
                        let next = pos + 1;
                        selection.set_selected(next);
                        list_view
                            .activate_action("list.scroll-to-item", Some(&next.to_variant()))
                            .ok();
                    }
                    glib::Propagation::Stop
                }
                Key::Up | Key::KP_Up => {
                    let pos = selection.selected();
                    if pos > 0 {
                        let prev = pos - 1;
                        selection.set_selected(prev);
                        list_view
                            .activate_action("list.scroll-to-item", Some(&prev.to_variant()))
                            .ok();
                    }
                    glib::Propagation::Stop
                }
                Key::Page_Down => {
                    let pos = selection.selected();
                    let n = store.n_items();
                    let next = (pos + 10).min(n.saturating_sub(1));
                    selection.set_selected(next);
                    list_view
                        .activate_action("list.scroll-to-item", Some(&next.to_variant()))
                        .ok();
                    glib::Propagation::Stop
                }
                Key::Page_Up => {
                    let pos = selection.selected();
                    let prev = pos.saturating_sub(10);
                    selection.set_selected(prev);
                    list_view
                        .activate_action("list.scroll-to-item", Some(&prev.to_variant()))
                        .ok();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        }
    ));
    entry.add_controller(key_ctrl);

    // ── Mouse click ───────────────────────────────────────────────────────────
    list_view.connect_activate(clone!(
        #[weak]
        window,
        #[strong]
        store,
        move |_, pos| {
            if let Some(item) = store.item(pos).and_then(|o| o.downcast::<AppItem>().ok()) {
                launch_app(&item.exec(), item.terminal());
            }
            window.close();
        }
    ));

    window.present();
    entry.grab_focus();
    populate("");
}

fn find_terminal() -> Option<String> {
    let candidates = [
        "foot",
        "alacritty",
        "kitty",
        "wezterm",
        "ghostty",
        "gnome-terminal",
        "xfce4-terminal",
        "konsole",
        "xterm",
    ];
    candidates
        .iter()
        .find(|&&term| {
            std::process::Command::new("which")
                .arg(term)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .map(|s| s.to_string())
}

fn power_action(action: &str) {
    match action {
        "logout" => logout_action(),
        "suspend" => {
            std::process::Command::new("systemctl")
                .arg("suspend")
                .spawn()
                .ok();
        }
        "reboot" => {
            std::process::Command::new("systemctl")
                .arg("reboot")
                .spawn()
                .ok();
        }
        "poweroff" => {
            std::process::Command::new("systemctl")
                .arg("poweroff")
                .spawn()
                .ok();
        }
        _ => {}
    }
}

fn logout_action() {
    // Try loginctl terminate-session with the real session ID from the environment
    if let Ok(session_id) = std::env::var("XDG_SESSION_ID") {
        if !session_id.is_empty() {
            let ok = std::process::Command::new("loginctl")
                .args(["terminate-session", &session_id])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                return;
            }
        }
    }
    // Fallback: terminate all sessions for the current user
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default();
    if !user.is_empty() {
        std::process::Command::new("loginctl")
            .args(["terminate-user", &user])
            .spawn()
            .ok();
    }
}

fn launch_app(exec: &str, terminal: bool) {
    let clean = launcher::clean_exec(exec);
    let cmd = if terminal {
        match find_terminal() {
            Some(term) => match term.as_str() {
                "gnome-terminal" | "xfce4-terminal" | "konsole" => {
                    format!("{} -- sh -c '{}'", term, clean)
                }
                _ => format!("{} -e sh -c '{}'", term, clean),
            },
            None => {
                eprintln!("No terminal emulator found");
                return;
            }
        }
    } else {
        clean
    };
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()
        .ok();
}
