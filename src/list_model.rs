use crate::app_item::AppItem;
use crate::calc_item::CalcItem;
use crate::calculator::{eval_expression, is_arithmetic_query};
use crate::cmd_item::CommandItem;
use crate::config::ObsidianConfig;
use crate::config::expand_home;
use crate::launcher::DesktopApp;
use crate::search_provider::{self, SearchProvider};
use crate::search_result_item::SearchResultItem;
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
use std::time::Duration;

#[derive(Clone)]
pub struct AppListModel {
    pub store: gio::ListStore,
    pub selection: SingleSelection,
    /// Populated asynchronously after the window is shown.
    all_apps: Rc<RefCell<Vec<DesktopApp>>>,
    /// The last query passed to `populate()`, so that `set_apps()` can
    /// re-run it once the app list has been loaded.
    current_query: Rc<RefCell<String>>,
    max_results: usize,
    calculator_enabled: bool,
    commands: Rc<HashMap<String, String>>,
    task_gen: Rc<Cell<u64>>,
    pub obsidian_cfg: Option<ObsidianConfig>,
    obsidian_action_mode: Rc<Cell<bool>>,
    obsidian_file_mode: Rc<Cell<bool>>,
    obsidian_grep_mode: Rc<Cell<bool>>,
    command_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    command_debounce_ms: u32,
    fuzzy_matcher: Rc<SkimMatcherV2>,
    search_providers: Rc<std::cell::OnceCell<Vec<SearchProvider>>>,
    search_provider_mode: Rc<Cell<bool>>,
}

impl AppListModel {
    /// Create a model with an empty app list. Call `set_apps()` once the
    /// background load has completed to populate and refresh the view.
    pub fn new(
        max_results: usize,
        calculator_enabled: bool,
        commands: HashMap<String, String>,
        obsidian_cfg: Option<ObsidianConfig>,
        command_debounce_ms: u32,
    ) -> Self {
        let store = gio::ListStore::new::<glib::Object>();
        let selection = SingleSelection::new(Some(store.clone()));
        selection.set_autoselect(true);
        selection.set_can_unselect(false);

        Self {
            store,
            selection,
            all_apps: Rc::new(RefCell::new(Vec::new())),
            current_query: Rc::new(RefCell::new(String::new())),
            max_results,
            calculator_enabled,
            commands: Rc::new(commands),
            task_gen: Rc::new(Cell::new(0)),
            obsidian_cfg,
            obsidian_action_mode: Rc::new(Cell::new(false)),
            obsidian_file_mode: Rc::new(Cell::new(false)),
            obsidian_grep_mode: Rc::new(Cell::new(false)),
            command_debounce: Rc::new(RefCell::new(None)),
            command_debounce_ms,
            fuzzy_matcher: Rc::new(SkimMatcherV2::default()),
            search_providers: Rc::new(std::cell::OnceCell::new()),
            search_provider_mode: Rc::new(Cell::new(false)),
        }
    }

    /// Replace the app list and re-run the current query so the view updates
    /// immediately without requiring any user interaction.
    pub fn set_apps(&self, apps: Vec<DesktopApp>) {
        *self.all_apps.borrow_mut() = apps;
        let query = self.current_query.borrow();
        if !query.starts_with(':') {
            self.populate(&query);
        }
    }

    fn cancel_debounce(&self) {
        if let Some(source_id) = self.command_debounce.borrow_mut().take() {
            let _ = source_id.remove();
        }
    }

    fn schedule_command_with_delay<F>(&self, delay_ms: u32, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel_debounce();
        let mut f_opt = Some(f);
        let debounce_ref = self.command_debounce.clone();
        let source_id =
            glib::timeout_add_local(Duration::from_millis(delay_ms.into()), move || {
                *debounce_ref.borrow_mut() = None;
                if let Some(f) = f_opt.take() {
                    f();
                }
                glib::ControlFlow::Break
            });
        *self.command_debounce.borrow_mut() = Some(source_id);
    }

    fn schedule_command<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.schedule_command_with_delay(self.command_debounce_ms, f);
    }

    fn run_subprocess(&self, mut cmd: std::process::Command) {
        let generation = self.task_gen.get();
        let max_results = self.max_results;
        let model_clone = self.clone();

        let (tx, rx) = std::sync::mpsc::channel::<Vec<String>>();

        std::thread::spawn(move || {
            let lines = cmd
                .output()
                .map(|out| {
                    String::from_utf8_lossy(&out.stdout)
                        .lines()
                        .take(max_results)
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let _ = tx.send(lines);
        });

        fn poll(rx: std::sync::mpsc::Receiver<Vec<String>>, model: AppListModel, generation: u64) {
            match rx.try_recv() {
                Ok(lines) => {
                    if model.task_gen.get() == generation {
                        model.store.remove_all();
                        for line in lines {
                            model.store.append(&CommandItem::new(line));
                        }
                        if model.store.n_items() > 0
                            && model.selection.selected() == gtk4::INVALID_LIST_POSITION
                        {
                            model.selection.set_selected(0);
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    glib::idle_add_local_once(move || poll(rx, model, generation));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
            }
        }
        glib::idle_add_local_once(move || poll(rx, model_clone, generation));
    }

    pub fn populate(&self, query: &str) {
        // Remember the query so set_apps() can replay it after the async load.
        *self.current_query.borrow_mut() = query.to_string();

        self.obsidian_action_mode.set(false);
        self.obsidian_file_mode.set(false);
        self.obsidian_grep_mode.set(false);
        self.search_provider_mode.set(false);

        self.cancel_debounce();

        // --- Colon command handling ---
        if query.starts_with(':') {
            let parts: Vec<&str> = query.splitn(2, ' ').collect();
            let cmd_part = parts.first().copied().unwrap_or(query);
            let arg = parts.get(1).unwrap_or(&"").trim();
            let cmd_name = &cmd_part[1..];

            // :s <query> — GNOME Shell search providers
            if cmd_name == "s" {
                if arg.is_empty() {
                    self.store.remove_all();
                    self.selection.set_selected(gtk4::INVALID_LIST_POSITION);
                    return;
                }
                let providers = self
                    .search_providers
                    .get_or_init(search_provider::discover_providers);
                if providers.is_empty() {
                    self.store.remove_all();
                    self.store.append(&crate::cmd_item::CommandItem::new(
                        "No GNOME Shell search providers found".to_string(),
                    ));
                    self.selection.set_selected(0);
                    return;
                }
                self.search_provider_mode.set(true);
                self.task_gen.set(self.task_gen.get() + 1);
                let providers_clone: Vec<SearchProvider> = providers.to_vec();
                let arg = arg.to_string();
                let max = self.max_results;
                let model_clone = self.clone();
                self.schedule_command_with_delay(120, move || {
                    model_clone.run_provider_search(providers_clone, arg, max);
                });
                return;
            }

            // Obsidian commands
            if cmd_name == "ob" || cmd_name == "obg" {
                let obs_cfg = match &self.obsidian_cfg {
                    Some(c) => c.clone(),
                    None => {
                        self.store.remove_all();
                        let item =
                            CommandItem::new("Obsidian not configured – edit config".to_string());
                        self.store.append(&item);
                        self.selection.set_selected(0);
                        return;
                    }
                };
                let vault_path =
                    expand_home(&obs_cfg.vault, &std::env::var("HOME").unwrap_or_default());
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
                            self.obsidian_action_mode.set(true);
                            self.store.remove_all();
                            self.selection.set_selected(gtk4::INVALID_LIST_POSITION);
                            return;
                        } else {
                            self.obsidian_file_mode.set(true);
                            self.task_gen.set(self.task_gen.get() + 1);
                            let vault_path = vault_path.to_string_lossy().to_string();
                            let arg = arg.to_string();
                            let model_clone = self.clone();
                            self.schedule_command(move || {
                                model_clone.run_find_in_vault(PathBuf::from(vault_path), &arg);
                            });
                            return;
                        }
                    }
                    "obg" => {
                        self.obsidian_grep_mode.set(true);
                        self.task_gen.set(self.task_gen.get() + 1);
                        let vault_path = vault_path.to_string_lossy().to_string();
                        let arg = arg.to_string();
                        let model_clone = self.clone();
                        self.schedule_command(move || {
                            model_clone.run_rg_in_vault(PathBuf::from(vault_path), &arg);
                        });
                        return;
                    }
                    _ => unreachable!(),
                }
            }

            // Regular colon commands (from config)
            if let Some(template) = (!self.commands.is_empty())
                .then(|| self.commands.get(cmd_name))
                .flatten()
            {
                self.task_gen.set(self.task_gen.get() + 1);
                let template = template.clone();
                let arg = arg.to_string();
                let cmd_name = cmd_name.to_string();
                let model_clone = self.clone();
                self.schedule_command(move || {
                    model_clone.run_command(&cmd_name, &template, &arg);
                });
                return;
            } else {
                return;
            }
        }

        // --- Non-colon query: clear and show apps/calculator ---
        self.store.remove_all();
        self.task_gen.set(self.task_gen.get() + 1);

        if self.calculator_enabled && !query.is_empty() && is_arithmetic_query(query) {
            if let Some(result_str) = eval_expression(query) {
                let calc_item = CalcItem::new(result_str);
                self.store.append(&calc_item);
            }
        }

        // Hold the borrow for the duration of the iteration.
        let apps = self.all_apps.borrow();

        if query.is_empty() {
            for app in apps.iter() {
                self.store.append(&AppItem::new(app));
            }
        } else {
            let mut results: Vec<(i64, &DesktopApp)> = apps
                .iter()
                .filter_map(|app| {
                    let name_score = self.fuzzy_matcher.fuzzy_match(&app.name, query);
                    let desc_score = if !app.description.is_empty() {
                        self.fuzzy_matcher
                            .fuzzy_match(&app.description, query)
                            .map(|s| s / 2)
                    } else {
                        None
                    };
                    let score = match (name_score, desc_score) {
                        (None, None) => return None,
                        (a, b) => a.unwrap_or(i64::MIN).max(b.unwrap_or(i64::MIN)),
                    };
                    Some((score, app))
                })
                .collect();

            results.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            results.truncate(self.max_results);

            for (_, app) in results {
                self.store.append(&AppItem::new(app));
            }
        }

        if self.store.n_items() > 0 {
            self.selection.set_selected(0);
        }
    }

    fn run_provider_search(&self, providers: Vec<SearchProvider>, query: String, max: usize) {
        let generation = self.task_gen.get();
        let model_clone = self.clone();
        let terms: Vec<String> = query.split_whitespace().map(String::from).collect();

        let clear_timeout = Rc::new(RefCell::new(None::<glib::SourceId>));
        let clear_model = self.clone();
        let clear_gen = generation;
        let clear_timeout_clone = clear_timeout.clone();
        let timeout_id = glib::timeout_add_local(Duration::from_millis(25), move || {
            if clear_model.task_gen.get() == clear_gen {
                clear_model.store.remove_all();
                clear_model
                    .selection
                    .set_selected(gtk4::INVALID_LIST_POSITION);
            }
            *clear_timeout_clone.borrow_mut() = None;
            glib::ControlFlow::Break
        });
        *clear_timeout.borrow_mut() = Some(timeout_id);

        let (tx, rx) = std::sync::mpsc::channel::<Vec<search_provider::SearchResult>>();
        std::thread::spawn(move || {
            search_provider::run_search_streaming(&providers, &query, max, tx);
        });

        let first_batch = Rc::new(Cell::new(false));
        fn poll(
            rx: std::sync::mpsc::Receiver<Vec<search_provider::SearchResult>>,
            model: AppListModel,
            generation: u64,
            terms: Vec<String>,
            clear_timeout: Rc<RefCell<Option<glib::SourceId>>>,
            first_batch: Rc<Cell<bool>>,
        ) {
            if model.task_gen.get() != generation {
                return;
            }
            loop {
                match rx.try_recv() {
                    Ok(results) => {
                        if model.task_gen.get() != generation {
                            return;
                        }
                        if let Some(id) = clear_timeout.borrow_mut().take() {
                            id.remove();
                        }

                        let items: Vec<glib::Object> = results
                            .into_iter()
                            .map(|r| {
                                let (icon_themed, icon_file) = match r.icon {
                                    Some(search_provider::IconData::Themed(n)) => {
                                        (n, String::new())
                                    }
                                    Some(search_provider::IconData::File(p)) => (String::new(), p),
                                    None => (String::new(), String::new()),
                                };
                                SearchResultItem::new(
                                    r.id,
                                    r.name,
                                    r.description,
                                    icon_themed,
                                    icon_file,
                                    r.app_icon,
                                    r.bus_name,
                                    r.object_path,
                                    terms.clone(),
                                )
                                .upcast::<glib::Object>()
                            })
                            .collect();

                        if !first_batch.get() {
                            model.store.remove_all();
                            first_batch.set(true);
                        }
                        model.store.splice(model.store.n_items(), 0, &items);

                        if model.store.n_items() > 0
                            && model.selection.selected() == gtk4::INVALID_LIST_POSITION
                        {
                            model.selection.set_selected(0);
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        glib::idle_add_local_once(move || {
                            poll(rx, model, generation, terms, clear_timeout, first_batch)
                        });
                        return;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        return;
                    }
                }
            }
        }

        glib::idle_add_local_once(move || {
            poll(
                rx,
                model_clone,
                generation,
                terms,
                clear_timeout,
                first_batch,
            )
        });
    }

    fn run_command(&self, _cmd_name: &str, template: &str, argument: &str) {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(template).arg("--").arg(argument);
        self.run_subprocess(cmd);
    }

    fn run_find_in_vault(&self, vault_path: PathBuf, pattern: &str) {
        let mut cmd = std::process::Command::new("find");
        cmd.arg(&vault_path)
            .arg("-type")
            .arg("f")
            .arg("-iname")
            .arg(format!("*{}*", pattern));
        self.run_subprocess(cmd);
    }

    fn run_rg_in_vault(&self, vault_path: PathBuf, pattern: &str) {
        let mut cmd = std::process::Command::new("rg");
        cmd.arg("--with-filename")
            .arg("--line-number")
            .arg("--no-heading")
            .arg("--color")
            .arg("never")
            .arg(pattern)
            .arg(&vault_path);
        self.run_subprocess(cmd);
    }

    pub fn create_factory(&self) -> SignalListItemFactory {
        let factory = SignalListItemFactory::new();

        let obsidian_file_mode = self.obsidian_file_mode.clone();
        let obsidian_grep_mode = self.obsidian_grep_mode.clone();

        let obsidian_icon = ["obsidian", "md.obsidian.Obsidian", "Obsidian"]
            .iter()
            .map(|id| crate::search_provider::resolve_app_icon(id))
            .find(|s| !s.is_empty())
            .unwrap_or_else(|| "text-x-markdown".to_string());

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

        factory.connect_bind(move |_, list_item| {
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
                let line = cmd_item.line();

                // Handle Obsidian grep mode first (special icon)
                if obsidian_grep_mode.get() {
                    image.set_icon_name(Some(&obsidian_icon));
                    name_label.set_text(&line);
                    desc_label.set_visible(false);
                    return;
                }

                // Try to parse as file path (absolute)
                if line.starts_with('/') {
                    // Check if it's a plain file path (no colon) – from :f or :ob find
                    if !line.contains(':') {
                        // Plain file path
                        if obsidian_file_mode.get() {
                            image.set_icon_name(Some(&obsidian_icon));
                        } else {
                            let (ctype, _) =
                                gtk4::gio::content_type_guess(Some(line.as_str()), None::<&[u8]>);
                            let icon = gtk4::gio::content_type_get_icon(&ctype);
                            image.set_from_gicon(&icon);
                        }
                        let filename = std::path::Path::new(&line)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(&line);
                        name_label.set_text(filename);
                        if let Some(parent) = std::path::Path::new(&line)
                            .parent()
                            .and_then(|p| p.to_str())
                        {
                            desc_label.set_visible(true);
                            desc_label.set_text(parent);
                        } else {
                            desc_label.set_visible(false);
                            desc_label.set_text("");
                        }
                        return;
                    } else {
                        // Line contains ':' – likely grep output from :fg or similar
                        if let Some((file_path, rest)) = line.split_once(':') {
                            let (ctype, _) =
                                gtk4::gio::content_type_guess(Some(file_path), None::<&[u8]>);
                            let icon = gtk4::gio::content_type_get_icon(&ctype);
                            image.set_from_gicon(&icon);

                            let filename = std::path::Path::new(file_path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(file_path);
                            name_label.set_text(filename);
                            desc_label.set_visible(true);
                            desc_label.set_text(rest);
                            return;
                        }
                    }
                }

                // Fallback for any other lines
                image.set_icon_name(Some("system-search"));
                name_label.set_text(&line);
                desc_label.set_visible(false);
            } else if let Some(sr_item) = obj.downcast_ref::<SearchResultItem>() {
                let icon_file = sr_item.icon_file();
                let icon_themed = sr_item.icon_themed();
                let app_icon = sr_item.app_icon_name();
                if !icon_file.is_empty() {
                    image.set_from_file(Some(&icon_file));
                } else if !icon_themed.is_empty() {
                    image.set_icon_name(Some(&icon_themed));
                } else if !app_icon.is_empty() {
                    image.set_icon_name(Some(&app_icon));
                } else {
                    image.set_icon_name(Some("system-search"));
                }
                name_label.set_text(&sr_item.name());
                let desc = sr_item.description();
                if desc.is_empty() {
                    desc_label.set_visible(false);
                    desc_label.set_text("");
                } else {
                    desc_label.set_visible(true);
                    desc_label.set_text(&desc);
                }
            } else {
                name_label.set_text("?");
                desc_label.set_visible(false);
            }
        });

        factory
    }

    pub fn obsidian_action_mode(&self) -> bool {
        self.obsidian_action_mode.get()
    }

    pub fn obsidian_file_mode(&self) -> bool {
        self.obsidian_file_mode.get()
    }
}
