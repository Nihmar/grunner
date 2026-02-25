use crate::app_item::AppItem;
use crate::bookmark_item::BookmarkItem;
use crate::bookmarks::{self, Bookmark};
use crate::calc_item::CalcItem;
use crate::calculator::{eval_expression, is_arithmetic_query};
use crate::clipboard_history::ClipboardHistory;
use crate::clipboard_item::ClipboardItem;
use crate::cmd_item::CommandItem;
use crate::config::ObsidianConfig;
use crate::config::expand_home;
use crate::history::LaunchHistory;
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
    all_apps: Rc<Vec<DesktopApp>>,
    max_results: usize,
    calculator_enabled: bool,
    commands: Rc<HashMap<String, String>>,
    task_gen: Rc<Cell<u64>>,
    pub obsidian_cfg: Option<ObsidianConfig>,
    obsidian_action_mode: Rc<Cell<bool>>,
    obsidian_file_mode: Rc<Cell<bool>>,
    command_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    command_debounce_ms: u32,
    fuzzy_matcher: Rc<SkimMatcherV2>,
    search_providers: Rc<std::cell::OnceCell<Vec<SearchProvider>>>,
    search_provider_mode: Rc<Cell<bool>>,
    // New config values
    clipboard_history_size: usize,
    enable_browser_bookmarks: bool,
}

impl AppListModel {
    pub fn new(
        all_apps: Rc<Vec<DesktopApp>>,
        max_results: usize,
        calculator_enabled: bool,
        commands: HashMap<String, String>,
        obsidian_cfg: Option<ObsidianConfig>,
        command_debounce_ms: u32,
        clipboard_history_size: usize,
        enable_browser_bookmarks: bool,
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
            commands: Rc::new(commands),
            task_gen: Rc::new(Cell::new(0)),
            obsidian_cfg,
            obsidian_action_mode: Rc::new(Cell::new(false)),
            obsidian_file_mode: Rc::new(Cell::new(false)),
            command_debounce: Rc::new(RefCell::new(None)),
            command_debounce_ms,
            fuzzy_matcher: Rc::new(SkimMatcherV2::default()),
            search_providers: Rc::new(std::cell::OnceCell::new()),
            search_provider_mode: Rc::new(Cell::new(false)),
            clipboard_history_size,
            enable_browser_bookmarks,
        }
    }

    // Cancel any pending debounced command
    fn cancel_debounce(&self) {
        if let Some(source_id) = self.command_debounce.borrow_mut().take() {
            let _ = source_id.remove();
        }
    }

    // Schedule a closure to run after the configured debounce delay;
    // cancels any previously scheduled command.
    fn schedule_command<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel_debounce();
        let mut f_opt = Some(f);
        let debounce_ref = self.command_debounce.clone();
        let source_id = glib::timeout_add_local(
            Duration::from_millis(self.command_debounce_ms.into()),
            move || {
                *debounce_ref.borrow_mut() = None;
                if let Some(f) = f_opt.take() {
                    f();
                }
                glib::ControlFlow::Break
            },
        );
        *self.command_debounce.borrow_mut() = Some(source_id);
    }

    // Shared helper: runs `cmd` on a background thread, then delivers its
    // stdout lines back to the GTK main thread.
    // This method is currently unused but kept for future functionality.
    #[allow(dead_code)]
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
                        if model.store.n_items() > 0 {
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
        // Reset mode flags
        self.obsidian_action_mode.set(false);
        self.obsidian_file_mode.set(false);
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
                self.schedule_command(move || {
                    model_clone.run_provider_search(providers_clone, arg, max);
                });
                return;
            }

            // :c — clipboard history
            if cmd_name == "c" {
                self.store.remove_all();
                let history = ClipboardHistory::load(Some(self.clipboard_history_size));
                for text in history.iter().rev() {
                    self.store.append(&ClipboardItem::new(text.clone()));
                }
                if self.store.n_items() > 0 {
                    self.selection.set_selected(0);
                }
                return;
            }

            // :b — browser bookmarks (if enabled)
            if self.enable_browser_bookmarks && cmd_name == "b" {
                self.store.remove_all();
                if arg.is_empty() {
                    return;
                }
                self.search_provider_mode.set(false);
                self.task_gen.set(self.task_gen.get() + 1);
                let arg = arg.to_string();
                let model_clone = self.clone();
                self.schedule_command(move || {
                    model_clone.run_bookmark_search(&arg);
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
                // Unknown command: keep previous list
                return;
            }
        }

        // --- Non-colon query ---
        self.store.remove_all();

        self.task_gen.set(self.task_gen.get() + 1);

        // Calculator (if enabled and query looks arithmetic)
        if self.calculator_enabled && !query.is_empty() && is_arithmetic_query(query) {
            if let Some(result_str) = eval_expression(query) {
                let calc_item = CalcItem::new(result_str);
                self.store.append(&calc_item);
            }
        }

        // Apps
        if query.is_empty() {
            // Show apps sorted by launch history
            let history = LaunchHistory::load();
            let mut apps_with_count: Vec<(&DesktopApp, u32)> = self
                .all_apps
                .iter()
                .map(|app| {
                    (
                        app,
                        history.get_count(&app.source_path.display().to_string()),
                    )
                })
                .collect();
            apps_with_count.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()))
            });
            for (app, _) in apps_with_count {
                self.store.append(&AppItem::new(app));
            }
        } else {
            let mut results: Vec<(i64, &DesktopApp)> = self
                .all_apps
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

    // New method for bookmark search
    fn run_bookmark_search(&self, query: &str) {
        let generation = self.task_gen.get();
        let model_clone = self.clone();
        let query = query.to_string();

        let (tx, rx) = std::sync::mpsc::channel::<Vec<Bookmark>>();

        std::thread::spawn(move || {
            let all = bookmarks::load_all_bookmarks();
            let matcher = SkimMatcherV2::default();
            let mut scored: Vec<(i64, Bookmark)> = all
                .into_iter()
                .filter_map(|b| {
                    let score = matcher
                        .fuzzy_match(&b.title, &query)
                        .or_else(|| matcher.fuzzy_match(&b.url, &query))
                        .unwrap_or(i64::MIN);
                    if score > i64::MIN {
                        Some((score, b))
                    } else {
                        None
                    }
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            let results: Vec<Bookmark> = scored.into_iter().map(|(_, b)| b).collect();
            let _ = tx.send(results);
        });

        fn poll(
            rx: std::sync::mpsc::Receiver<Vec<Bookmark>>,
            model: AppListModel,
            generation: u64,
        ) {
            match rx.try_recv() {
                Ok(bookmarks) => {
                    if model.task_gen.get() == generation {
                        model.store.remove_all();
                        for b in bookmarks {
                            let item = BookmarkItem::new(b.title, b.url);
                            model.store.append(&item);
                        }
                        if model.store.n_items() > 0 {
                            model.selection.set_selected(0);
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    glib::idle_add_local_once(move || poll(rx, model, generation));
                }
                Err(_) => {}
            }
        }
        glib::idle_add_local_once(move || poll(rx, model_clone, generation));
    }

    // Stubbed methods – may be implemented later
    fn run_provider_search(&self, _providers: Vec<SearchProvider>, _query: String, _max: usize) {
        // TODO: implement actual provider search
    }

    fn run_command(&self, _cmd_name: &str, _template: &str, _argument: &str) {
        // TODO: implement command execution
    }

    fn run_find_in_vault(&self, _vault_path: PathBuf, _pattern: &str) {
        // TODO: implement `find` search in Obsidian vault
    }

    fn run_rg_in_vault(&self, _vault_path: PathBuf, _pattern: &str) {
        // TODO: implement `rg` search in Obsidian vault
    }

    // create_factory – fixed indentation and icon placeholders
    pub fn create_factory(&self) -> SignalListItemFactory {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(|_, _list_item| {
            // Setup is handled in bind; nothing needed here.
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
                // FIXME: replace with actual icon method when available
                image.set_icon_name(Some("application-x-executable"));
                name_label.set_text(&app_item.name());
                if !app_item.description().is_empty() {
                    desc_label.set_visible(true);
                    desc_label.set_text(&app_item.description());
                } else {
                    desc_label.set_visible(false);
                    desc_label.set_text("");
                }
            } else if let Some(calc_item) = obj.downcast_ref::<CalcItem>() {
                image.set_icon_name(Some("accessories-calculator"));
                name_label.set_text(&calc_item.result());
                desc_label.set_visible(false);
                desc_label.set_text("");
            } else if let Some(cmd_item) = obj.downcast_ref::<CommandItem>() {
                image.set_icon_name(Some("utilities-terminal"));
                name_label.set_text(&cmd_item.line());
                desc_label.set_visible(false);
                desc_label.set_text("");
            } else if let Some(sr_item) = obj.downcast_ref::<SearchResultItem>() {
                // FIXME: replace with actual icon method when trait bounds are satisfied
                image.set_icon_name(Some("system-search"));
                name_label.set_text(&sr_item.name());
                if !sr_item.description().is_empty() {
                    desc_label.set_visible(true);
                    desc_label.set_text(&sr_item.description());
                } else {
                    desc_label.set_visible(false);
                    desc_label.set_text("");
                }
            } else if let Some(clip_item) = obj.downcast_ref::<ClipboardItem>() {
                image.set_icon_name(Some("edit-paste"));
                name_label.set_text(&clip_item.text());
                desc_label.set_visible(false);
                desc_label.set_text("");
            } else if let Some(bm_item) = obj.downcast_ref::<BookmarkItem>() {
                image.set_icon_name(Some("text-html"));
                name_label.set_text(&bm_item.title());
                let url = bm_item.url();
                if !url.is_empty() {
                    desc_label.set_visible(true);
                    desc_label.set_text(&url);
                } else {
                    desc_label.set_visible(false);
                    desc_label.set_text("");
                }
            } else {
                name_label.set_text("?");
                desc_label.set_visible(false);
            }
        });

        factory
    }

    // Public getters
    pub fn obsidian_action_mode(&self) -> bool {
        self.obsidian_action_mode.get()
    }
    pub fn obsidian_file_mode(&self) -> bool {
        self.obsidian_file_mode.get()
    }
    // This getter is currently unused but kept for consistency.
    #[allow(dead_code)]
    pub fn search_provider_mode(&self) -> bool {
        self.search_provider_mode.get()
    }
}
