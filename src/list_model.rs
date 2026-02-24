use crate::app_item::AppItem;
use crate::calc_item::CalcItem;
use crate::calculator::{eval_expression, is_arithmetic_query};
use crate::cmd_item::CommandItem;
use crate::config::ObsidianConfig;
use crate::config::expand_home;
use crate::launcher::DesktopApp;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use glib::object::Cast;
use gtk4::gio;
use gtk4::prelude::*;
use gtk4::{ListItem, SignalListItemFactory, SingleSelection};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Clone)]
pub struct AppListModel {
    pub store: gio::ListStore,
    pub selection: SingleSelection,
    all_apps: Rc<Vec<DesktopApp>>,
    max_results: usize,
    calculator_enabled: bool,
    commands: HashMap<String, String>,
    task_gen: Rc<Cell<u64>>,
    pub obsidian_cfg: Option<ObsidianConfig>,
    // flag to indicate that the Obsidian action buttons should be shown
    obsidian_action_mode: Rc<Cell<bool>>,
    // debounce timer for colon commands
    command_debounce: Rc<RefCell<Option<glib::SourceId>>>,
}

impl AppListModel {
    pub fn new(
        all_apps: Rc<Vec<DesktopApp>>,
        max_results: usize,
        calculator_enabled: bool,
        commands: HashMap<String, String>,
        obsidian_cfg: Option<ObsidianConfig>,
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
            obsidian_cfg,
            obsidian_action_mode: Rc::new(Cell::new(false)),
            command_debounce: Rc::new(RefCell::new(None)),
        }
    }

    // Cancel any pending debounced command
    fn cancel_debounce(&self) {
        if let Some(source_id) = self.command_debounce.borrow_mut().take() {
            source_id.remove();
        }
    }

    // Schedule a closure to run after `delay_ms`; cancels any previously scheduled
    fn schedule_command<F>(&self, delay_ms: u32, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel_debounce();
        let source_id = glib::timeout_add_local(delay_ms, move || {
            f();
            glib::ControlFlow::Break
        });
        *self.command_debounce.borrow_mut() = Some(source_id);
    }

    pub fn populate(&self, query: &str) {
        // Reset Obsidian action mode at the start of every population
        self.obsidian_action_mode.set(false);

        // Cancel any pending command debounce (will be rescheduled if needed)
        self.cancel_debounce();

        // --- Colon command handling ---
        if query.starts_with(':') && !self.commands.is_empty() {
            let parts: Vec<&str> = query.splitn(2, ' ').collect();
            let cmd_part = parts[0];
            let arg = parts.get(1).unwrap_or(&"").trim();
            let cmd_name = &cmd_part[1..];

            // Special handling for obsidian commands
            if cmd_name == "ob" || cmd_name == "obg" {
                // Check if obsidian is configured
                let obs_cfg = match &self.obsidian_cfg {
                    Some(c) => c.clone(),
                    None => {
                        self.store.remove_all();
                        let item = CommandItem::new("Obsidian not configured – edit config".to_string());
                        self.store.append(&item);
                        self.selection.set_selected(0);
                        return;
                    }
                };
                let vault_path = expand_home(&obs_cfg.vault, &std::env::var("HOME").unwrap_or_default());
                if !vault_path.exists() {
                    self.store.remove_all();
                    let item = CommandItem::new(format!(
                        "Vault path does not exist: {}",
                        vault_path.display()
                    ));
                    self.store.append(&item);
                    self.selection.set_selected(0);
                    return;
                }

                match cmd_name {
                    "ob" => {
                        if arg.is_empty() {
                            // Show buttons immediately, clear list
                            self.obsidian_action_mode.set(true);
                            self.store.remove_all();
                            self.selection.set_selected(gtk4::INVALID_LIST_POSITION);
                            return;
                        } else {
                            // Schedule find search
                            let vault_path = vault_path.to_string_lossy().to_string();
                            let arg = arg.to_string();
                            let model_clone = self.clone();
                            self.schedule_command(300, move || {
                                model_clone.run_find_in_vault(PathBuf::from(vault_path), &arg);
                            });
                            return; // Do NOT clear the list yet
                        }
                    }
                    "obg" => {
                        // Schedule rg search
                        let vault_path = vault_path.to_string_lossy().to_string();
                        let arg = arg.to_string();
                        let model_clone = self.clone();
                        self.schedule_command(300, move || {
                            model_clone.run_rg_in_vault(PathBuf::from(vault_path), &arg);
                        });
                        return; // Do NOT clear the list yet
                    }
                    _ => unreachable!(),
                }
            }

            // Regular colon commands (from config)
            if let Some(template) = self.commands.get(cmd_name) {
                let template = template.clone();
                let arg = arg.to_string();
                let model_clone = self.clone();
                self.schedule_command(300, move || {
                    model_clone.run_command(cmd_name, &template, &arg);
                });
                return; // Do NOT clear the list yet
            } else {
                // Unknown command: do nothing, keep the previous list
                return;
            }
        }

        // --- Non-colon query: clear and show apps/calculator ---
        self.store.remove_all();

        // Increment generation to cancel previous async tasks
        self.task_gen.set(self.task_gen.get() + 1);

        // Calculator (if enabled and query looks arithmetic)
        if self.calculator_enabled && !query.is_empty() && is_arithmetic_query(query) {
            if let Some(result_str) = eval_expression(query) {
                let calc_item = CalcItem::new(result_str);
                self.store.append(&calc_item);
            }
        }

        // Apps (fuzzy search)
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

    fn run_find_in_vault(&self, vault_path: PathBuf, pattern: &str) {
        let generation = self.task_gen.get();
        let max_results = self.max_results;
        let vault_path = vault_path.to_string_lossy().to_string();
        let pattern = pattern.to_string();
        let model_clone = self.clone();

        let (tx, rx) = std::sync::mpsc::channel::<Vec<String>>();

        std::thread::spawn(move || {
            let output = std::process::Command::new("find")
                .arg(&vault_path)
                .arg("-type")
                .arg("f")
                .arg("-iname")
                .arg(format!("*{}*", pattern))
                .arg("-printf")
                .arg("%P\\n") // relative path from vault
                .output();

            let lines = match output {
                Ok(out) => String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .take(max_results)
                    .map(|line| vault_path.clone() + "/" + line) // reconstruct full path
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

    fn run_rg_in_vault(&self, vault_path: PathBuf, pattern: &str) {
        let generation = self.task_gen.get();
        let max_results = self.max_results;
        let vault_path = vault_path.to_string_lossy().to_string();
        let pattern = pattern.to_string();
        let model_clone = self.clone();

        let (tx, rx) = std::sync::mpsc::channel::<Vec<String>>();

        std::thread::spawn(move || {
            let output = std::process::Command::new("rg")
                .arg("--with-filename")
                .arg("--line-number")
                .arg("--no-heading")
                .arg("--color")
                .arg("never")
                .arg(&pattern)
                .arg(&vault_path)
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

    // Public getter for the Obsidian action mode flag
    pub fn obsidian_action_mode(&self) -> bool {
        self.obsidian_action_mode.get()
    }
}