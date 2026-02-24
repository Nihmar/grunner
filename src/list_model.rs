use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::thread::spawn;

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use glib::idle_add_local_once;
use glib::object::Cast;
use gtk4::gio;
use gtk4::prelude::*;
use gtk4::{ListItem, SignalListItemFactory, SingleSelection};

use crate::app_item::AppItem;
use crate::calc_item::CalcItem;
use crate::calculator::{eval_expression, is_arithmetic_query};
use crate::cmd_item::CommandItem;
use crate::launcher::DesktopApp;

#[derive(Clone)]
pub struct AppListModel {
    pub store: gio::ListStore,
    pub selection: SingleSelection,
    all_apps: Rc<Vec<DesktopApp>>,
    max_results: usize,
    calculator_enabled: bool,
    commands: HashMap<String, String>,
    task_gen: Rc<Cell<u64>>, // to cancel stale async tasks
}

impl AppListModel {
    pub fn new(
        all_apps: Rc<Vec<DesktopApp>>,
        max_results: usize,
        calculator_enabled: bool,
        commands: HashMap<String, String>,
    ) -> Self {
        let store = gio::ListStore::new::<glib::Object>();
        let selection = SingleSelection::new(Some(store.clone()));
        selection.set_autoselect(true);
        selection.set_can_unselect(false);

        Self {
            store,
            selection,
            all_apps,
            max_results,
            calculator_enabled,
            commands,
            task_gen: Rc::new(Cell::new(0)),
        }
    }

    pub fn populate(&self, query: &str) {
        // Increment generation to cancel previous async tasks
        self.task_gen.set(self.task_gen.get() + 1);
        self.store.remove_all();

        // --- Colon command handling ---
        if query.starts_with(':') && !self.commands.is_empty() {
            let parts: Vec<&str> = query.splitn(2, ' ').collect();
            let cmd_part = parts[0]; // e.g. ":f"
            let arg = parts.get(1).unwrap_or(&"").trim();
            let cmd_name = &cmd_part[1..]; // remove ':'
            if let Some(template) = self.commands.get(cmd_name) {
                self.run_command(cmd_name, template, arg);
                return; // No apps shown while command is running
            } else {
                // Unknown command – show nothing
                return;
            }
        }

        // --- Calculator (only if enabled and query looks arithmetic) ---
        if self.calculator_enabled && !query.is_empty() && is_arithmetic_query(query) {
            if let Some(result_str) = eval_expression(query) {
                let calc_item = CalcItem::new(result_str);
                self.store.append(&calc_item);
            }
        }

        // --- Apps (fuzzy search) ---
        if query.is_empty() {
            for app in self.all_apps.iter() {
                self.store.append(&AppItem::new(app));
            }
        } else {
            let matcher = SkimMatcherV2::default();
            let mut results: Vec<(i64, &DesktopApp)> = self
                .all_apps
                .iter()
                .filter_map(|app| {
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
                })
                .collect();

            results.sort_by(|a, b| b.0.cmp(&a.0));
            results.truncate(self.max_results);

            for (_, app) in results {
                self.store.append(&AppItem::new(app));
            }
        }

        if self.store.n_items() > 0 {
            self.selection.set_selected(0);
        }
    }

    fn run_command(&self, _cmd_name: &str, template: &str, argument: &str) {
        let generation = self.task_gen.get();
        let max_results = self.max_results;
        let template = template.to_string();
        let argument = argument.to_string();
        let model_clone = self.clone();

        let (tx, rx) = std::sync::mpsc::channel::<Vec<String>>();

        std::thread::spawn(move || {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&template)
                .arg("--")
                .arg(&argument)
                .output();

            let lines = match output {
                Ok(out) => String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .take(max_results)
                    .map(String::from)
                    .collect::<Vec<_>>(),
                Err(_) => Vec::new(),
            };

            let _ = tx.send(lines);
        });

        glib::idle_add_local(move || match rx.try_recv() {
            Ok(lines) => {
                if model_clone.task_gen.get() == generation {
                    model_clone.store.remove_all();
                    for line in lines {
                        let item = CommandItem::new(line);
                        model_clone.store.append(&item);
                    }
                    if model_clone.store.n_items() > 0 {
                        model_clone.selection.set_selected(0);
                    }
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        });
    }

    pub fn create_factory() -> SignalListItemFactory {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<ListItem>().unwrap();

            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
            hbox.set_margin_top(6);
            hbox.set_margin_bottom(6);
            hbox.set_margin_start(12);
            hbox.set_margin_end(12);

            let image = gtk4::Image::new();
            image.set_pixel_size(32);
            image.set_valign(gtk4::Align::Center);
            image.add_css_class("app-icon");
            hbox.append(&image);

            let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
            vbox.set_valign(gtk4::Align::Center);
            vbox.set_hexpand(true);

            let name_label = gtk4::Label::new(None);
            name_label.set_halign(gtk4::Align::Start);
            name_label.add_css_class("row-name");
            vbox.append(&name_label);

            let desc_label = gtk4::Label::new(None);
            desc_label.set_halign(gtk4::Align::Start);
            desc_label.add_css_class("row-desc");
            desc_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            desc_label.set_max_width_chars(70);
            vbox.append(&desc_label);

            hbox.append(&vbox);
            list_item.set_child(Some(&hbox));
        });

        factory.connect_bind(|_, list_item| {
            let list_item = list_item.downcast_ref::<ListItem>().unwrap();
            let obj = match list_item.item() {
                Some(o) => o,
                None => return,
            };

            let hbox = list_item
                .child()
                .and_then(|c| c.downcast::<gtk4::Box>().ok())
                .expect("missing hbox");
            let image = hbox
                .first_child()
                .and_then(|c| c.downcast::<gtk4::Image>().ok())
                .expect("missing image");
            let vbox = image
                .next_sibling()
                .and_then(|c| c.downcast::<gtk4::Box>().ok())
                .expect("missing vbox");
            let name_label = vbox
                .first_child()
                .and_then(|c| c.downcast::<gtk4::Label>().ok())
                .expect("missing name_label");
            let desc_label = name_label
                .next_sibling()
                .and_then(|c| c.downcast::<gtk4::Label>().ok())
                .expect("missing desc_label");

            if let Some(app_item) = obj.downcast_ref::<AppItem>() {
                let icon = app_item.icon();
                if icon.is_empty() {
                    image.set_icon_name(Some("application-x-executable"));
                } else if icon.starts_with('/') {
                    image.set_from_file(Some(&icon));
                } else {
                    image.set_icon_name(Some(&icon));
                }
                name_label.set_text(&app_item.name());
                let desc = app_item.description();
                if desc.is_empty() {
                    desc_label.set_visible(false);
                    desc_label.set_text("");
                } else {
                    desc_label.set_visible(true);
                    desc_label.set_text(&desc);
                }
            } else if let Some(calc_item) = obj.downcast_ref::<CalcItem>() {
                image.set_icon_name(Some("accessories-calculator"));
                name_label.set_text(&calc_item.result());
                desc_label.set_visible(false);
                desc_label.set_text("");
            } else if let Some(cmd_item) = obj.downcast_ref::<CommandItem>() {
                image.set_icon_name(Some("system-search"));
                name_label.set_text(&cmd_item.line());
                desc_label.set_visible(false);
                desc_label.set_text("");
            } else {
                name_label.set_text("?");
                desc_label.set_visible(false);
            }
        });

        factory
    }
}
