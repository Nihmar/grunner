//! GTK list model and data management for Grunner
//!
//! This module provides the main data model that powers the search UI:
//! - Application list management with fuzzy matching
//! - Command execution and result handling
//! - Search provider integration
//! - Real-time result updates with background threads
//!
//! The `AppListModel` struct coordinates three sub-components:
//! - [`SearchState`]: manages query text and task generation for cancellation
//! - [`DebounceScheduler`]: handles debounce timers for commands and search
//! - `ModelConfig`: holds configuration (`max_results`, obsidian, commands, blacklist)

use crate::app_mode::ActiveMode;
use crate::core::config::{CommandConfig, ObsidianConfig};
use crate::launcher::DesktopApp;
use crate::model::items::SearchResultItem;
use crate::providers::dbus::{self, SearchProvider as DbusSearchProvider};
use crate::providers::{AppProvider, CalculatorProvider, SearchProvider};
use gtk4::SingleSelection;
use gtk4::gio;
use gtk4::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

const DEFAULT_SEARCH_DEBOUNCE_MS: u32 = 100;
const PROVIDER_SEARCH_DEBOUNCE_MS: u32 = 120;
const PROVIDER_CLEAR_TIMEOUT_MS: u64 = 25;

// ── Search State ─────────────────────────────────────────────────────────────

/// Manages search state: current query and task generation for cancellation.
///
/// Task generation allows stale async operations to be detected and discarded
/// when the user types new input before previous searches complete.
#[derive(Clone)]
pub struct SearchState {
    current_query: Rc<RefCell<String>>,
    task_gen: Rc<Cell<u64>>,
    active_mode: Rc<Cell<ActiveMode>>,
}

impl SearchState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_query: Rc::new(RefCell::new(String::new())),
            task_gen: Rc::new(Cell::new(0)),
            active_mode: Rc::new(Cell::new(ActiveMode::None)),
        }
    }

    #[must_use]
    pub fn current_query(&self) -> String {
        self.current_query.borrow().clone()
    }

    pub fn set_query(&self, query: &str) {
        *self.current_query.borrow_mut() = query.to_string();
    }

    #[must_use]
    pub fn active_mode(&self) -> ActiveMode {
        self.active_mode.get()
    }

    pub fn set_active_mode(&self, mode: ActiveMode) {
        self.active_mode.set(mode);
    }

    #[must_use]
    pub fn bump_task_gen(&self) -> u64 {
        let next = self.task_gen.get() + 1;
        self.task_gen.set(next);
        next
    }

    #[must_use]
    pub fn task_gen(&self) -> u64 {
        self.task_gen.get()
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Debounce Scheduler ────────────────────────────────────────────────────────

/// Manages debounce timers for command execution and search operations.
///
/// Provides separate scheduling for:
/// - Commands (colon commands) using `schedule_command`
/// - Search providers using `schedule_search`
pub struct DebounceScheduler {
    command_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    command_debounce_ms: Cell<u32>,
    search_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    search_debounce_ms: u32,
}

impl DebounceScheduler {
    #[must_use]
    pub fn new(command_ms: u32, search_ms: u32) -> Self {
        Self {
            command_debounce: Rc::new(RefCell::new(None)),
            command_debounce_ms: Cell::new(command_ms),
            search_debounce: Rc::new(RefCell::new(None)),
            search_debounce_ms: search_ms,
        }
    }

    #[must_use]
    pub fn command_debounce_ms(&self) -> u32 {
        self.command_debounce_ms.get()
    }

    pub fn set_command_debounce_ms(&self, ms: u32) {
        self.command_debounce_ms.set(ms);
    }

    pub fn cancel_command(&self) {
        if let Some(id) = self.command_debounce.borrow_mut().take() {
            id.remove();
        }
    }

    pub fn cancel_search(&self) {
        if let Some(id) = self.search_debounce.borrow_mut().take() {
            id.remove();
        }
    }

    pub fn schedule_command<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel_command();
        Self::schedule_with_delay(&self.command_debounce, self.command_debounce_ms.get(), f);
    }

    pub fn schedule_search<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel_search();
        Self::schedule_with_delay(&self.search_debounce, self.search_debounce_ms, f);
    }

    pub fn schedule_command_with_delay<F>(&self, delay_ms: u32, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel_command();
        Self::schedule_with_delay(&self.command_debounce, delay_ms, f);
    }

    fn schedule_with_delay<F>(slot: &Rc<RefCell<Option<glib::SourceId>>>, delay_ms: u32, f: F)
    where
        F: FnOnce() + 'static,
    {
        if let Some(id) = slot.borrow_mut().take() {
            id.remove();
        }
        let mut f_opt = Some(f);
        let slot_clone = slot.clone();
        let source_id =
            glib::timeout_add_local(Duration::from_millis(delay_ms.into()), move || {
                *slot_clone.borrow_mut() = None;
                if let Some(f) = f_opt.take() {
                    f();
                }
                glib::ControlFlow::Break
            });
        *slot.borrow_mut() = Some(source_id);
    }
}

impl Clone for DebounceScheduler {
    fn clone(&self) -> Self {
        Self {
            command_debounce: Rc::clone(&self.command_debounce),
            command_debounce_ms: Cell::new(self.command_debounce_ms.get()),
            search_debounce: Rc::clone(&self.search_debounce),
            search_debounce_ms: self.search_debounce_ms,
        }
    }
}

// ── Model Config ─────────────────────────────────────────────────────────────

/// Holds configuration settings for the search model.
///
/// Contains values that can be updated via `apply_config` without recreating the model.
#[derive(Clone)]
pub struct ModelConfig {
    pub max_results: Cell<usize>,
    pub obsidian_cfg: Option<ObsidianConfig>,
    pub commands: Rc<RefCell<Vec<CommandConfig>>>,
    pub blacklist: Rc<RefCell<Vec<String>>>,
    pub disable_modes: Cell<bool>,
    pub providers: Rc<Vec<Box<dyn SearchProvider>>>,
}

impl ModelConfig {
    pub fn new(
        max_results: usize,
        obsidian_cfg: Option<ObsidianConfig>,
        blacklist: Vec<String>,
        commands: Vec<CommandConfig>,
        disable_modes: bool,
        all_apps: Rc<RefCell<Vec<DesktopApp>>>,
    ) -> Self {
        let providers = Rc::new(vec![
            Box::new(AppProvider::new(all_apps, max_results)) as Box<dyn SearchProvider>,
            Box::new(CalculatorProvider::new()) as Box<dyn SearchProvider>,
        ]);

        Self {
            max_results: Cell::new(max_results),
            obsidian_cfg,
            commands: Rc::new(RefCell::new(commands)),
            blacklist: Rc::new(RefCell::new(blacklist)),
            disable_modes: Cell::new(disable_modes),
            providers,
        }
    }

    pub fn apply_config(&self, config: &crate::core::config::Config) {
        self.max_results.set(config.max_results);
        self.disable_modes.set(config.disable_modes);

        for provider in self.providers.iter() {
            provider.set_max_results(config.max_results);
        }

        (*self.blacklist.borrow_mut()).clone_from(&config.search_provider_blacklist);
        (*self.commands.borrow_mut()).clone_from(&config.commands);
    }
}

// ── Pollers ───────────────────────────────────────────────────────────────────

/// Drives the idle-poll loop for a streaming search-provider query.
///
/// This struct handles the more complex streaming results from
/// GNOME Shell search providers, which can return multiple batches
/// of results over time.
struct ProviderSearchPoller {
    /// Channel receiver for search result batches
    rx: std::sync::mpsc::Receiver<Vec<dbus::SearchResult>>,
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
        if self.model.state.task_gen() != self.generation {
            return;
        }

        // Consume all currently-available batches before yielding back to the
        // main loop, so a fast provider doesn't stall behind repeated idles.
        let this = self;
        loop {
            match this.rx.try_recv() {
                Ok(results) => {
                    // Double-check generation after receiving results
                    if this.model.state.task_gen() != this.generation {
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
                                Some(dbus::IconData::Themed(n)) => (n, String::new()),
                                Some(dbus::IconData::File(p)) => (String::new(), p),
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
/// Coordinates three sub-components:
/// - [`SearchState`]: query text and task generation for cancellation
/// - [`DebounceScheduler`]: debounce timers for commands and search
/// - `ModelConfig`: configuration (`max_results`, obsidian, commands, blacklist)
///
/// The struct itself provides GTK list/selection models and delegates
/// to the sub-components.
#[derive(Clone)]
pub struct AppListModel {
    pub(crate) store: gio::ListStore,
    pub(crate) selection: SingleSelection,

    pub(crate) state: SearchState,
    pub(crate) debounce: DebounceScheduler,
    pub(crate) config: ModelConfig,

    /// Cached GNOME Shell search providers
    search_providers: Rc<std::cell::OnceCell<Vec<DbusSearchProvider>>>,
    /// All available desktop applications (used by providers)
    all_apps: Rc<RefCell<Vec<DesktopApp>>>,
}

/// Trait for command handlers that need to interact with the list model.
///
/// This abstraction allows `CommandHandler` to be tested with mock implementations
/// and reduces coupling between command logic and the concrete model type.
pub trait CommandSink: Clone + 'static {
    fn mode(&self) -> ActiveMode;
    fn set_mode(&self, mode: ActiveMode);
    fn clear(&self);
    fn push(&self, item: &impl IsA<glib::Object>);
    fn count(&self) -> u32;
    fn select(&self, pos: u32);
    fn bump_gen(&self) -> u64;
    fn schedule<F: FnOnce() + 'static>(&self, f: F);
    fn bump_and_schedule<F: FnOnce() + 'static>(&self, f: F);
    fn get_commands(&self, query: &str) -> Vec<CommandConfig>;
    fn obsidian_config(&self) -> Option<ObsidianConfig>;
}

impl CommandSink for AppListModel {
    fn mode(&self) -> ActiveMode {
        self.active_mode()
    }
    fn set_mode(&self, mode: ActiveMode) {
        self.set_active_mode(mode);
    }

    fn clear(&self) {
        self.remove_all_store_items();
    }

    fn push(&self, item: &impl IsA<glib::Object>) {
        self.append_store_item(item);
    }

    fn count(&self) -> u32 {
        self.store_item_count()
    }

    fn select(&self, pos: u32) {
        self.set_selected_position(pos);
    }

    fn bump_gen(&self) -> u64 {
        self.bump_task_gen()
    }

    fn schedule<F: FnOnce() + 'static>(&self, f: F) {
        AppListModel::schedule_command(self, f);
    }

    fn bump_and_schedule<F: FnOnce() + 'static>(&self, f: F) {
        self.bump_task_gen();
        AppListModel::schedule_command(self, f);
    }

    fn get_commands(&self, query: &str) -> Vec<CommandConfig> {
        let commands = self.config.commands.borrow();
        commands
            .iter()
            .filter(|cmd| {
                if query.is_empty() {
                    true
                } else {
                    cmd.name.to_lowercase().contains(&query.to_lowercase())
                        || cmd.command.to_lowercase().contains(&query.to_lowercase())
                }
            })
            .cloned()
            .collect()
    }

    fn obsidian_config(&self) -> Option<ObsidianConfig> {
        self.config.obsidian_cfg.clone()
    }
}

impl AppListModel {
    // ── Internal API ──────────────────────────────────────────────────────────

    /// Set the current active mode
    pub(crate) fn set_active_mode(&self, mode: ActiveMode) {
        self.state.set_active_mode(mode);
    }

    /// Get the current active mode
    pub(crate) fn active_mode(&self) -> ActiveMode {
        self.state.active_mode()
    }

    /// Append an item to the list store
    pub(crate) fn append_store_item(&self, obj: &impl IsA<glib::Object>) {
        self.store.append(obj);
    }

    /// Remove all items from the list store
    pub(crate) fn remove_all_store_items(&self) {
        self.store.remove_all();
    }

    /// Return the number of items in the list store
    pub(crate) fn store_item_count(&self) -> u32 {
        self.store.n_items()
    }

    /// Set the selected position in the selection model
    pub(crate) fn set_selected_position(&self, pos: u32) {
        self.selection.set_selected(pos);
    }

    /// Return a reference to the Obsidian configuration, if present
    pub(crate) fn obsidian_config(&self) -> Option<&ObsidianConfig> {
        self.config.obsidian_cfg.as_ref()
    }

    /// Create a new `AppListModel` with the given configuration
    ///
    /// # Arguments
    /// * `max_results` - Maximum number of search results to display
    /// * `obsidian_cfg` - Optional Obsidian configuration
    /// * `command_debounce_ms` - Debounce delay for command execution
    /// * `search_provider_blacklist` - List of provider IDs to exclude
    /// * `commands` - List of custom script commands
    /// * `disable_modes` - Whether to disable all special modes (colon commands)
    #[must_use]
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

        let all_apps = Rc::new(RefCell::new(Vec::new()));

        let state = SearchState::new();
        let debounce = DebounceScheduler::new(command_debounce_ms, DEFAULT_SEARCH_DEBOUNCE_MS);
        let config = ModelConfig::new(
            max_results,
            obsidian_cfg,
            search_provider_blacklist,
            commands,
            disable_modes,
            all_apps.clone(),
        );

        Self {
            store,
            selection,
            state,
            debounce,
            config,
            search_providers: Rc::new(std::cell::OnceCell::new()),
            all_apps,
        }
    }

    /// Update the list of available desktop applications
    ///
    /// This is typically called once at startup after scanning .desktop files.
    /// It triggers a repopulation of the list with the current query.
    pub fn set_apps(&self, apps: Vec<DesktopApp>) {
        *self.all_apps.borrow_mut() = apps;
        let query = self.state.current_query();
        self.populate(&query);
    }

    /// Apply configuration changes (hot-reload after saving settings)
    ///
    /// This updates all configurable settings without restarting the app.
    pub fn apply_config(&self, config: &crate::core::config::Config) {
        let old_max_results = self.config.max_results.get();

        self.config.apply_config(config);

        // Update command debounce
        self.debounce
            .set_command_debounce_ms(config.command_debounce_ms);

        // Repopulate if max_results changed or in CustomScript mode
        if old_max_results != config.max_results {
            let query = self.state.current_query();
            self.populate(&query);
        } else if self.state.active_mode() == ActiveMode::CustomScript {
            use crate::command_handler::CommandHandler;
            let query = self.state.current_query();
            let handler = CommandHandler::new(self.clone());
            handler.handle_sh(&query);
        }
    }

    /// Cancel any pending command debounce timer
    ///
    /// Used when the user types new input before a delayed command executes.
    fn cancel_debounce(&self) {
        self.debounce.cancel_command();
    }

    fn cancel_search_debounce(&self) {
        self.debounce.cancel_search();
    }

    /// Schedule a command to run with the configured default debounce delay
    pub(crate) fn schedule_command<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.debounce.schedule_command(f);
    }

    fn schedule_search<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.debounce.schedule_search(f);
    }

    /// Schedule a search provider query to run with a specific delay
    fn schedule_provider_search_with_delay<F>(&self, delay_ms: u32, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.debounce.schedule_command_with_delay(delay_ms, f);
    }

    pub(crate) fn bump_task_gen(&self) -> u64 {
        self.state.bump_task_gen()
    }

    /// Bump the task generation counter and schedule a command that
    /// only executes if no newer bump has occurred.
    pub(crate) fn bump_and_schedule<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        let generation = self.bump_task_gen();
        let model_clone = self.clone();
        self.schedule_command(move || {
            if model_clone.state.task_gen() == generation {
                f();
            }
        });
    }

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
            // Default search: debounce via schedule_search (uses search_debounce_ms)
            self.schedule_search(move || model.populate(&query));
        }
    }

    /// Main entry point for updating search results based on query
    ///
    /// This method routes the query to the appropriate handler:
    /// - Colon commands (starting with `:`) go to command handlers
    /// - Empty queries show all applications
    /// - Non-empty queries trigger fuzzy application search
    pub fn populate(&self, query: &str) {
        self.state.set_query(query);
        self.state.set_active_mode(ActiveMode::None);
        self.cancel_debounce();
        self.cancel_search_debounce();

        // Handle colon-prefixed commands (skip if modes are disabled)
        if !self.config.disable_modes.get() && query.starts_with(':') {
            self.handle_colon_command(query);
            return;
        }

        // Regular application search
        self.store.remove_all();
        self.bump_task_gen();

        // Use providers for standard search
        let mut all_results: Vec<glib::Object> = Vec::new();

        for provider in self.config.providers.iter() {
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

    /// Schedule a search provider query to run in parallel with application search
    fn schedule_provider_search(&self, query: String, clear_store: bool) {
        // Discover providers (cached after first use)
        let providers = self
            .search_providers
            .get_or_init(|| dbus::discover_providers(&self.config.blacklist.borrow()));

        if providers.is_empty() {
            return;
        }

        self.state.set_active_mode(ActiveMode::None);
        self.bump_task_gen();
        let providers_clone: Vec<DbusSearchProvider> = providers.clone();
        let max = self.config.max_results.get();
        let model_clone = self.clone();
        // Use shorter debounce for search providers for more responsive feel
        self.schedule_provider_search_with_delay(PROVIDER_SEARCH_DEBOUNCE_MS, move || {
            model_clone.run_provider_search(providers_clone, query, max, clear_store);
        });
    }

    /// Handle colon-prefixed commands by routing to appropriate handlers
    fn handle_colon_command(&self, query: &str) {
        use crate::command_handler::CommandHandler;
        let handler = CommandHandler::new(self.clone());
        handler.handle_colon_command(query);
    }

    /// Run a search query through GNOME Shell search providers
    fn run_provider_search(
        &self,
        providers: Vec<DbusSearchProvider>,
        query: String,
        max: usize,
        clear_store: bool,
    ) {
        let generation = self.state.task_gen();
        let model_clone = self.clone();
        let terms: Vec<String> = query.split_whitespace().map(String::from).collect();

        // Set up a short timeout to clear old results and show "searching" state
        let clear_timeout = Rc::new(RefCell::new(None::<glib::SourceId>));
        if clear_store {
            let clear_model = self.clone();
            let clear_gen = generation;
            let clear_timeout_clone = clear_timeout.clone();
            let timeout_id = glib::timeout_add_local(
                Duration::from_millis(PROVIDER_CLEAR_TIMEOUT_MS),
                move || {
                    if clear_model.state.task_gen() == clear_gen {
                        clear_model.store.remove_all();
                        clear_model
                            .selection
                            .set_selected(gtk4::INVALID_LIST_POSITION);
                    }
                    *clear_timeout_clone.borrow_mut() = None;
                    glib::ControlFlow::Break
                },
            );
            *clear_timeout.borrow_mut() = Some(timeout_id);
        }

        // Channel for streaming results from background thread
        let (tx, rx) = std::sync::mpsc::channel::<Vec<dbus::SearchResult>>();
        std::thread::spawn(move || {
            dbus::run_search_streaming(&providers, &query, max, tx);
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
}

impl Default for DebounceScheduler {
    fn default() -> Self {
        Self::new(300, DEFAULT_SEARCH_DEBOUNCE_MS)
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::is_calculator_result;

    #[test]
    fn test_is_calculator_result() {
        assert!(is_calculator_result("2 + 2 = 4"));
        assert!(is_calculator_result("10 / 2 = 5"));
        assert!(is_calculator_result("sin(0) = 0"));
        assert!(is_calculator_result("cos(0) = 1"));
        assert!(is_calculator_result("sqrt(4) = 2"));
        assert!(is_calculator_result("pi = 3.1415926536"));
        assert!(is_calculator_result("e = 2.7182818285"));
        assert!(!is_calculator_result("abc"));
        assert!(!is_calculator_result("2 + 2"));
        assert!(!is_calculator_result(""));
    }
}
