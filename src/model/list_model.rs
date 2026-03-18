//! GTK list model and data management for Grunner
//!
//! This module provides the main data model that powers the search UI:
//! - Application list management with fuzzy matching
//! - Command execution and result handling
//! - Search provider integration
//! - Obsidian vault searching
//! - Real-time result updates with background threads
//!
//! The `AppListModel` struct is the central coordinator that manages
//! all search modes, executes commands, and updates the GTK list store.

use crate::actions::which;
use crate::app_mode::ActiveMode;
use crate::core::config::{CommandConfig, ObsidianConfig};
use crate::core::global_state::get_home_dir;
use crate::launcher::DesktopApp;
use crate::model::items::CommandItem;
use crate::model::items::SearchResultItem;
use crate::providers::dbus_provider::{self, SearchProvider as DbusSearchProvider};
use crate::providers::{AppProvider, CalculatorProvider, SearchProvider};
use crate::utils::expand_home;
use gtk4::gio;
use gtk4::prelude::*;
use gtk4::{SignalListItemFactory, SingleSelection};
use log::{debug, error};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

const DEFAULT_SEARCH_DEBOUNCE_MS: u32 = 100;

/// Parse a colon-prefixed command into command name and argument
///
/// Colon commands follow the format ":command argument" where:
/// - `:` is the command prefix
/// - `command` is the command name (e.g., "f", "ob", "s")
/// - `argument` is the optional search argument (trimmed)
///
/// # Examples
/// - `":f foo"` → `("f", "foo")`
/// - `":ob"` → `("ob", "")`
/// - `":obg pattern"` → `("obg", "pattern")`
fn parse_colon_command(query: &str) -> (&str, &str) {
    let rest = &query[1..];
    match rest.split_once(' ') {
        Some((cmd, arg)) => (cmd, arg.trim()),
        None => (rest, ""),
    }
}

// ── Pollers ───────────────────────────────────────────────────────────────────

/// Drives the idle-poll loop for a plain subprocess result (`run_subprocess`).
///
/// This struct manages the asynchronous collection of command output
/// from background threads, updating the UI when results are ready.
struct SubprocessPoller {
    /// Channel receiver for command output lines
    rx: std::sync::mpsc::Receiver<Vec<String>>,
    /// Reference to the main list model for UI updates
    model: AppListModel,
    /// Generation ID to prevent stale updates after new searches
    generation: u64,
}

impl SubprocessPoller {
    /// Poll for subprocess results and update UI when ready
    ///
    /// This method checks for available output from the background thread
    /// and updates the list store if the generation still matches.
    /// If no data is ready yet, it schedules itself to run again on idle.
    fn poll(self) {
        match self.rx.try_recv() {
            Ok(lines) => {
                // Only update if this poller is still for the current search
                if self.model.task_gen.get() == self.generation {
                    self.model.store.remove_all();
                    for line in lines {
                        self.model.store.append(&CommandItem::new(line));
                    }
                    // Auto-select first item if nothing is selected
                    if self.model.store.n_items() > 0
                        && self.model.selection.selected() == gtk4::INVALID_LIST_POSITION
                    {
                        self.model.selection.set_selected(0);
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // No data yet - reschedule poll on next idle
                glib::idle_add_local_once(move || self.poll());
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // Thread finished without sending data (empty output or error)
            }
        }
    }
}

/// Drives the idle-poll loop for a streaming search-provider query.
///
/// This struct handles the more complex streaming results from
/// GNOME Shell search providers, which can return multiple batches
/// of results over time.
struct ProviderSearchPoller {
    /// Channel receiver for search result batches
    rx: std::sync::mpsc::Receiver<Vec<dbus_provider::SearchResult>>,
    /// Reference to the main list model for UI updates
    model: AppListModel,
    /// Generation ID to prevent stale updates after new searches
    generation: u64,
    /// Search terms for highlighting in results
    terms: Vec<String>,
    /// Timeout source ID for clearing old results
    clear_timeout: Rc<RefCell<Option<glib::SourceId>>>,
    /// Whether the first batch of results has been processed
    first_batch: Rc<Cell<bool>>,
    /// Whether to clear the store before showing results
    clear_store: bool,
}

impl ProviderSearchPoller {
    /// Poll for search provider results and update UI as batches arrive
    ///
    /// This method processes available result batches in a loop to avoid
    /// unnecessary idle callbacks when providers return data quickly.
    /// It handles the first batch specially by clearing previous results
    /// and manages a timeout to show a loading indicator.
    fn poll(self) {
        // Early exit if search generation has changed
        if self.model.task_gen.get() != self.generation {
            return;
        }

        // Consume all currently-available batches before yielding back to the
        // main loop, so a fast provider doesn't stall behind repeated idles.
        let this = self;
        loop {
            match this.rx.try_recv() {
                Ok(results) => {
                    // Double-check generation after receiving results
                    if this.model.task_gen.get() != this.generation {
                        return;
                    }

                    // Cancel the clear timeout since we now have results
                    if let Some(id) = this.clear_timeout.borrow_mut().take() {
                        id.remove();
                    }

                    // Convert search results to GTK list items
                    let items: Vec<glib::Object> = results
                        .into_iter()
                        .map(|r| {
                            let (icon_themed, icon_file) = match r.icon {
                                Some(dbus_provider::IconData::Themed(n)) => (n, String::new()),
                                Some(dbus_provider::IconData::File(p)) => (String::new(), p),
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
                                this.terms.clone(),
                            )
                            .upcast::<glib::Object>()
                        })
                        .collect();

                    // Clear store only on first batch and if clear_store is true
                    if !this.first_batch.get() && this.clear_store {
                        this.model.store.remove_all();
                        this.first_batch.set(true);
                    }

                    // Append new items to the store
                    this.model
                        .store
                        .splice(this.model.store.n_items(), 0, &items);

                    // Auto-select first item if nothing is selected
                    if this.model.store.n_items() > 0
                        && this.model.selection.selected() == gtk4::INVALID_LIST_POSITION
                    {
                        this.model.selection.set_selected(0);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // No more data ready - schedule next poll on idle
                    glib::idle_add_local_once(move || this.poll());
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // Provider finished sending all results
                    return;
                }
            }
        }
    }
}

/// Main data model for Grunner's search interface
///
/// This struct manages all aspects of search functionality:
/// - Application listing and fuzzy search
/// - Command execution and result display
/// - Search provider integration
/// - Obsidian vault searching
/// - Result caching and UI synchronization
#[derive(Clone)]
pub struct AppListModel {
    /// GTK list store containing the current search results
    pub store: gio::ListStore,
    /// GTK selection model for tracking selected item
    pub selection: SingleSelection,

    /// All available desktop applications (cached)
    all_apps: Rc<RefCell<Vec<DesktopApp>>>,

    /// Current search query text
    current_query: Rc<RefCell<String>>,
    /// Maximum number of results to show
    max_results: Cell<usize>,

    /// Generation counter for cancelling stale async tasks
    task_gen: Rc<Cell<u64>>,
    /// Obsidian configuration (if enabled)
    pub obsidian_cfg: Option<ObsidianConfig>,
    /// Current active mode for UI rendering
    active_mode: Rc<Cell<ActiveMode>>,
    /// Debounce timer source ID for delayed command execution
    command_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    /// Debounce delay in milliseconds
    command_debounce_ms: Cell<u32>,
    /// Debounce timer source ID for delayed search execution
    search_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    /// Debounce delay in milliseconds for default search mode
    search_debounce_ms: u32,
    /// Cached GNOME Shell search providers
    search_providers: Rc<std::cell::OnceCell<Vec<DbusSearchProvider>>>,
    /// List of search provider IDs to exclude
    search_provider_blacklist: Rc<RefCell<Vec<String>>>,
    /// List of custom script commands
    commands: Rc<RefCell<Vec<crate::core::config::CommandConfig>>>,
    /// Whether all special modes (colon commands) are disabled
    disable_modes: bool,
    /// Search providers for different search types
    providers: Rc<Vec<Box<dyn SearchProvider>>>,
}

impl AppListModel {
    /// Create a new AppListModel with the given configuration
    ///
    /// # Arguments
    /// * `max_results` - Maximum number of search results to display
    /// * `obsidian_cfg` - Optional Obsidian configuration
    /// * `command_debounce_ms` - Debounce delay for command execution
    /// * `search_provider_blacklist` - List of provider IDs to exclude
    /// * `commands` - List of custom script commands
    /// * `disable_modes` - Whether to disable all special modes (colon commands)
    pub fn new(
        max_results: usize,
        obsidian_cfg: Option<ObsidianConfig>,
        command_debounce_ms: u32,
        search_provider_blacklist: Vec<String>,
        commands: Vec<crate::core::config::CommandConfig>,
        disable_modes: bool,
    ) -> Self {
        let store = gio::ListStore::new::<glib::Object>();
        let selection = SingleSelection::new(Some(store.clone()));
        selection.set_autoselect(true);
        selection.set_can_unselect(false);

        // Initialize search providers
        let all_apps = Rc::new(RefCell::new(Vec::new()));
        let commands_rc = Rc::new(RefCell::new(commands));
        let blacklist_rc = Rc::new(RefCell::new(search_provider_blacklist));

        let providers = Rc::new(vec![
            Box::new(AppProvider::new(all_apps.clone(), max_results)) as Box<dyn SearchProvider>,
            Box::new(CalculatorProvider::new()) as Box<dyn SearchProvider>,
            // CommandProvider is used only in :sh mode, handled separately
            // DbusSearchProvider is handled separately due to async nature
        ]);

        Self {
            store,
            selection,
            all_apps,
            current_query: Rc::new(RefCell::new(String::new())),
            max_results: Cell::new(max_results),

            task_gen: Rc::new(Cell::new(0)),
            obsidian_cfg,
            active_mode: Rc::new(Cell::new(ActiveMode::None)),
            command_debounce: Rc::new(RefCell::new(None)),
            command_debounce_ms: Cell::new(command_debounce_ms),
            search_debounce: Rc::new(RefCell::new(None)),
            search_debounce_ms: DEFAULT_SEARCH_DEBOUNCE_MS,
            search_providers: Rc::new(std::cell::OnceCell::new()),
            search_provider_blacklist: blacklist_rc,
            commands: commands_rc,
            disable_modes,
            providers,
        }
    }

    /// Update the list of available desktop applications
    ///
    /// This is typically called once at startup after scanning .desktop files.
    /// It triggers a repopulation of the list with the current query.
    pub fn set_apps(&self, apps: Vec<DesktopApp>) {
        *self.all_apps.borrow_mut() = apps;
        let query = self.current_query.borrow().clone();
        self.populate(&query);
    }

    /// Apply configuration changes (hot-reload after saving settings)
    ///
    /// This updates all configurable settings without restarting the app.
    pub fn apply_config(&self, config: &crate::core::config::Config) {
        // Update max_results
        self.max_results.set(config.max_results);

        // Update command debounce
        self.command_debounce_ms.set(config.command_debounce_ms);

        // Update search provider blacklist
        *self.search_provider_blacklist.borrow_mut() = config.search_provider_blacklist.clone();

        // Update commands
        *self.commands.borrow_mut() = config.commands.clone();

        // Repopulate if in CustomScript mode
        if self.active_mode.get() == ActiveMode::CustomScript {
            let query = self.current_query.borrow().clone();
            self.handle_sh(&query);
        }
    }

    /// Cancel any pending command debounce timer
    ///
    /// Used when the user types new input before a delayed command executes.
    fn cancel_debounce(&self) {
        if let Some(id) = self.command_debounce.borrow_mut().take() {
            id.remove();
        }
    }

    fn cancel_search_debounce(&self) {
        if let Some(id) = self.search_debounce.borrow_mut().take() {
            id.remove();
        }
    }

    /// Schedule a command to run after a delay with debouncing
    ///
    /// # Arguments
    /// * `delay_ms` - Delay in milliseconds before executing the command
    /// * `f` - Closure to execute (typically a command runner)
    ///
    /// This cancels any existing debounce timer and sets up a new one,
    /// ensuring commands only run after the user has stopped typing.
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

    fn schedule_search_with_delay<F>(&self, delay_ms: u32, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel_search_debounce();
        let mut f_opt = Some(f);
        let debounce_ref = self.search_debounce.clone();
        let source_id =
            glib::timeout_add_local(Duration::from_millis(delay_ms.into()), move || {
                *debounce_ref.borrow_mut() = None;
                if let Some(f) = f_opt.take() {
                    f();
                }
                glib::ControlFlow::Break
            });
        *self.search_debounce.borrow_mut() = Some(source_id);
    }

    /// Schedule a command to run with the configured default debounce delay
    fn schedule_command<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.schedule_command_with_delay(self.command_debounce_ms.get(), f);
    }

    fn schedule_search<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.schedule_search_with_delay(self.search_debounce_ms, f);
    }

    /// Schedule a search provider query to run in parallel with application search
    ///
    /// This mimics GNOME Search behavior where search provider results appear
    /// alongside application results when filtering.
    fn schedule_provider_search(&self, query: String, clear_store: bool) {
        // Discover providers (cached after first use)
        let providers = self.search_providers.get_or_init(|| {
            dbus_provider::discover_providers(&self.search_provider_blacklist.borrow())
        });

        if providers.is_empty() {
            return;
        }

        self.active_mode.set(ActiveMode::None);
        self.bump_task_gen();
        let providers_clone: Vec<DbusSearchProvider> = providers.clone();
        let max = self.max_results.get();
        let model_clone = self.clone();
        // Use shorter debounce for search providers for more responsive feel
        self.schedule_command_with_delay(120, move || {
            model_clone.run_provider_search(providers_clone, query, max, clear_store);
        });
    }

    /// Increment the task generation counter and return the new value
    ///
    /// This is used to identify stale async tasks - if a task's generation
    /// doesn't match the current one, its results should be discarded.
    fn bump_task_gen(&self) -> u64 {
        let next_gen = self.task_gen.get() + 1;
        self.task_gen.set(next_gen);
        next_gen
    }

    /// Display an error message as the only item in the list
    ///
    /// Used for configuration errors, missing dependencies, etc.
    fn show_error_item(&self, msg: impl Into<String>) {
        self.store.remove_all();
        self.store.append(&CommandItem::new(msg.into()));
        self.selection.set_selected(0);
    }

    /// Clear all items from the list store and reset selection
    fn clear_store(&self) {
        self.store.remove_all();
        self.selection.set_selected(gtk4::INVALID_LIST_POSITION);
    }
    /// Main entry point for updating search results based on query
    ///
    /// This method routes the query to the appropriate handler:
    /// - Colon commands (starting with `:`) go to command handlers
    /// - Empty queries show all applications
    /// - Non-empty queries trigger fuzzy application search
    pub fn populate(&self, query: &str) {
        *self.current_query.borrow_mut() = query.to_string();
        self.active_mode.set(ActiveMode::None);
        self.cancel_debounce();
        self.cancel_search_debounce();

        // Handle colon-prefixed commands (skip if modes are disabled)
        if !self.disable_modes && query.starts_with(':') {
            self.handle_colon_command(query);
            return;
        }

        // Regular application search
        self.store.remove_all();
        self.bump_task_gen();

        // Use providers for standard search
        let mut all_results: Vec<glib::Object> = Vec::new();

        for provider in self.providers.iter() {
            let mut results = provider.search(query);
            all_results.append(&mut results);
        }

        // Add results to store
        for item in all_results {
            self.store.append(&item);
        }

        // Schedule search provider query to mimic GNOME Search behavior
        if !query.is_empty() {
            self.schedule_provider_search(query.to_string(), false);
        }

        // Auto-select first item if we have results
        if self.store.n_items() > 0 {
            self.selection.set_selected(0);
        }
    }

    /// Schedule a populate call with debounce for default search mode
    ///
    /// This method should be called from UI when the search query changes.
    /// It will cancel any pending search debounce and schedule a new one.
    /// Colon commands are handled immediately without debounce.
    pub fn schedule_populate(&self, query: &str) {
        self.cancel_debounce();
        self.cancel_search_debounce();

        // Empty query: immediate clear
        if query.is_empty() {
            let model = self.clone();
            glib::idle_add_local_once(move || model.populate(""));
            return;
        }

        let query = query.to_string();
        let model = self.clone();

        if query.starts_with(':') {
            // Colon commands: immediate (they have internal debounce)
            glib::idle_add_local_once(move || model.populate(&query));
        } else {
            // Default search: 200ms debounce
            self.schedule_search(move || model.populate(&query));
        }
    }

    /// Handle colon-prefixed commands by routing to appropriate handlers
    fn handle_colon_command(&self, query: &str) {
        let (cmd_part, arg) = parse_colon_command(query);
        debug!("handle_colon_command: query='{query}', cmd_part='{cmd_part}', arg='{arg}'");
        debug!("Active mode: {:?}", self.active_mode.get());

        match cmd_part {
            "ob" | "obg" => self.handle_obsidian(cmd_part, arg),
            "f" => self.handle_file_search(arg),
            "fg" => self.handle_file_grep(arg),
            "sh" => {
                debug!("Calling handle_sh with arg: '{arg}'");
                self.handle_sh(arg);
            }
            _ => {
                if !cmd_part.is_empty() {
                    self.show_error_item(format!("Unknown command: :{cmd_part}"));
                }
            }
        }
    }

    /// Validate the Obsidian vault path from configuration
    ///
    /// Returns `Some(PathBuf)` if vault is configured and exists,
    /// otherwise shows an error and returns `None`.
    fn validated_vault_path(&self) -> Option<PathBuf> {
        let obs_cfg = if let Some(c) = &self.obsidian_cfg {
            c.clone()
        } else {
            self.show_error_item("Obsidian not configured - edit config");
            return None;
        };
        let vault_path = expand_home(&obs_cfg.vault);
        if !vault_path.exists() {
            self.show_error_item(format!(
                "Vault path does not exist: {}",
                vault_path.display()
            ));
            return None;
        }
        Some(vault_path)
    }

    /// Handle Obsidian search modes triggered by `:ob` and `:obg` commands
    fn handle_obsidian(&self, cmd_name: &str, arg: &str) {
        let Some(vault_path) = self.validated_vault_path() else {
            return;
        };
        let vault_str = vault_path.to_string_lossy().into_owned();

        let (mode, runner): (ActiveMode, Box<dyn FnOnce()>) = match (cmd_name, arg.is_empty()) {
            ("ob", true) => {
                // Empty :ob command - show Obsidian action mode
                self.active_mode.set(ActiveMode::ObsidianAction);
                self.clear_store();
                return;
            }
            ("obg", true) => {
                // Empty :obg command - show Obsidian grep mode
                self.active_mode.set(ActiveMode::ObsidianGrep);
                self.clear_store();
                return;
            }
            ("ob", false) => {
                // :ob with argument - file search in vault
                let arg = arg.to_string();
                let model_clone = self.clone();
                (
                    ActiveMode::ObsidianFile,
                    Box::new(move || model_clone.run_find_in_vault(PathBuf::from(vault_str), &arg)),
                )
            }
            ("obg", false) => {
                // :obg with argument - ripgrep (with grep fallback) search in vault
                let arg = arg.to_string();
                let model_clone = self.clone();
                (
                    ActiveMode::ObsidianGrep,
                    Box::new(move || model_clone.run_rg_in_vault(PathBuf::from(vault_str), &arg)),
                )
            }
            _ => {
                // Should never happen as cmd_name comes from known commands
                error!("Unexpected obsidian command: {cmd_name}");
                return;
            }
        };

        self.active_mode.set(mode);
        self.bump_task_gen();
        self.schedule_command(runner);
    }

    fn handle_file_search(&self, arg: &str) {
        if arg.is_empty() {
            self.clear_store();
            return;
        }

        self.bump_task_gen();
        let arg = arg.to_string();
        let model_clone = self.clone();
        self.schedule_command(move || {
            model_clone.run_file_search(&arg);
        });
    }

    fn handle_file_grep(&self, arg: &str) {
        if arg.is_empty() {
            self.clear_store();
            return;
        }

        self.bump_task_gen();
        let arg = arg.to_string();
        let model_clone = self.clone();
        self.schedule_command(move || {
            model_clone.run_file_grep(&arg);
        });
    }

    fn handle_sh(&self, arg: &str) {
        debug!("Setting active_mode to CustomScript");
        self.active_mode.set(ActiveMode::CustomScript);
        self.clear_store();

        debug!(
            "handle_sh called with arg: '{}', commands count: {}",
            arg,
            self.commands.borrow().len()
        );

        // Filter saved commands based on argument
        let commands = self.commands.borrow();
        let filtered_commands: Vec<CommandConfig> = commands
            .iter()
            .filter(|cmd| {
                if arg.is_empty() {
                    true
                } else {
                    // Simple substring match for name or command
                    cmd.name.to_lowercase().contains(&arg.to_lowercase())
                        || cmd.command.to_lowercase().contains(&arg.to_lowercase())
                }
            })
            .cloned()
            .collect();

        debug!("Filtered commands count: {}", filtered_commands.len());

        // Add filtered commands to store
        for cmd in filtered_commands {
            // Format as "Name | Command" for display
            let item_str = format!("{} | {}", cmd.name, cmd.command);
            self.store.append(&CommandItem::new_with_options(
                item_str,
                cmd.working_dir.clone(),
                cmd.keep_open,
            ));
        }

        // If user typed a command that doesn't match saved ones, add "Run: ..." option
        if !arg.is_empty() {
            let run_item_str = format!("Run: {arg}");
            // Custom commands default to keep_open=true
            self.store
                .append(&CommandItem::new_with_options(run_item_str, None, true));
        }

        debug!("Final store count: {}", self.store.n_items());
        debug!("Active mode is now: {:?}", self.active_mode.get());
    }

    /// Run a subprocess command and collect its output in a background thread
    ///
    /// The command output is sent back to the main thread via a channel,
    /// then processed by a `SubprocessPoller` to update the UI.
    fn run_subprocess(&self, mut cmd: std::process::Command) {
        let generation = self.task_gen.get();
        let max_results = self.max_results.get();
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

        let poller = SubprocessPoller {
            rx,
            model: model_clone,
            generation,
        };
        glib::idle_add_local_once(move || poller.poll());
    }

    /// Run a search query through GNOME Shell search providers
    ///
    /// This method coordinates the complex streaming search process:
    /// 1. Sets up a timeout to show "searching..." indicator
    /// 2. Spawns a background thread to query all providers
    /// 3. Sets up a poller to receive streaming results
    fn run_provider_search(
        &self,
        providers: Vec<DbusSearchProvider>,
        query: String,
        max: usize,
        clear_store: bool,
    ) {
        let generation = self.task_gen.get();
        let model_clone = self.clone();
        let terms: Vec<String> = query.split_whitespace().map(String::from).collect();

        // Set up a short timeout to clear old results and show "searching" state
        let clear_timeout = Rc::new(RefCell::new(None::<glib::SourceId>));
        if clear_store {
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
        }

        // Channel for streaming results from background thread
        let (tx, rx) = std::sync::mpsc::channel::<Vec<dbus_provider::SearchResult>>();
        std::thread::spawn(move || {
            dbus_provider::run_search_streaming(&providers, &query, max, tx);
        });

        let poller = ProviderSearchPoller {
            rx,
            model: model_clone,
            generation,
            terms,
            clear_timeout,
            first_batch: Rc::new(Cell::new(false)),
            clear_store,
        };
        glib::idle_add_local_once(move || poller.poll());
    }

    /// Execute a file search command without using shell
    fn run_file_search(&self, argument: &str) {
        // Try plocate first, fall back to find
        let command = if which("plocate").is_some() {
            // plocate -i -- "$argument" 2>/dev/null
            let mut cmd = std::process::Command::new("plocate");
            cmd.arg("-i")
                .arg("--")
                .arg(argument)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());
            cmd
        } else {
            // find "$HOME" -type f -ipath "*$argument*" 2>/dev/null
            let home = get_home_dir();
            let mut cmd = std::process::Command::new("find");
            cmd.arg(home)
                .arg("-type")
                .arg("f")
                .arg("-iname")
                .arg(format!("*{argument}*"))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());
            cmd
        };

        self.run_subprocess(command);
    }

    /// Execute a file grep command without using shell
    fn run_file_grep(&self, argument: &str) {
        let command = if which("rg").is_some() {
            // rg --with-filename --line-number --no-heading -S "$argument" ~ 2>/dev/null | head -20
            let home = get_home_dir();
            let mut cmd = std::process::Command::new("rg");
            cmd.arg("--with-filename")
                .arg("--line-number")
                .arg("--no-heading")
                .arg("-i")
                .arg(argument)
                .arg(home)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());
            cmd
        } else {
            // grep -r -i -n -I -H -- "$argument" "$HOME" 2>/dev/null | head -20
            let home = get_home_dir();
            let mut cmd = std::process::Command::new("grep");
            cmd.arg("-r")
                .arg("-i")
                .arg("-n")
                .arg("-I")
                .arg("-H")
                .arg("--")
                .arg(argument)
                .arg(home)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());
            cmd
        };

        // Run the command (output limited by run_subprocess)
        self.run_subprocess(command);
    }

    /// Run `find` command to search for files in Obsidian vault
    fn run_find_in_vault(&self, vault_path: PathBuf, pattern: &str) {
        let mut cmd = std::process::Command::new("find");
        cmd.arg(&vault_path)
            .arg("-type")
            .arg("f")
            .arg("-iname")
            .arg(format!("*{pattern}*"));
        self.run_subprocess(cmd);
    }

    /// Run `rg` (ripgrep with grep fallback) command to search file contents in Obsidian vault
    fn run_rg_in_vault(&self, vault_path: PathBuf, pattern: &str) {
        if which("rg").is_some() {
            let mut cmd = std::process::Command::new("rg");
            cmd.arg("-i")
                .arg("--with-filename")
                .arg("--line-number")
                .arg("--no-heading")
                .arg("--color=never")
                .arg(pattern)
                .arg(&vault_path);
            self.run_subprocess(cmd);
        } else {
            let mut cmd = std::process::Command::new("grep");
            cmd.arg("-r")
                .arg("-n")
                .arg("-i")
                .arg("-I")
                .arg("-H")
                .arg("--color=never")
                .arg("--")
                .arg(pattern)
                .arg(&vault_path);
            self.run_subprocess(cmd);
        }
    }

    /// Create a GTK SignalListItemFactory for rendering list items
    ///
    /// This factory uses the external list_factory module to handle
    /// UI creation and binding, separating presentation logic from data management.
    pub fn create_factory(&self) -> SignalListItemFactory {
        let active_mode = self.active_mode.get();
        let vault_path = self
            .obsidian_cfg
            .as_ref()
            .map(|cfg| expand_home(&cfg.vault).to_string_lossy().into_owned());
        crate::ui::list_factory::create_factory(active_mode, vault_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::is_calculator_result;

    #[test]
    fn test_is_calculator_result() {
        // Test basic calculator results
        assert!(is_calculator_result("2 + 2 = 4"));
        assert!(is_calculator_result("10 / 2 = 5"));

        // Test function results
        assert!(is_calculator_result("sin(0) = 0"));
        assert!(is_calculator_result("cos(0) = 1"));
        assert!(is_calculator_result("sqrt(4) = 2"));
        assert!(is_calculator_result("tan(0) = 0"));

        // Test constant results
        assert!(is_calculator_result("pi = 3.1415926536"));
        assert!(is_calculator_result("e = 2.7182818285"));

        // Test complex expressions with functions
        assert!(is_calculator_result("sin(0 + 0) = 0"));
        assert!(is_calculator_result("sqrt(2 + 2) = 2"));

        // Test invalid results (no equals sign or wrong format)
        assert!(!is_calculator_result("abc"));
        assert!(!is_calculator_result("2 + 2"));
        assert!(!is_calculator_result(""));

        // Note: is_calculator_result only checks format, not validity of expression
        // "sin(x) = 1" has valid format (letters, parentheses, equals, number)
        // but would fail at evaluation because 'x' is not a recognized identifier
        // This is correct behavior - the function identifies potential calculator results,
        // not validated results
    }
}
