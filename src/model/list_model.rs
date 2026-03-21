//! GTK list model and data management for Grunner
//!
//! This module provides the main data model that powers the search UI:
//! - Application list management with fuzzy matching
//! - Command execution and result handling
//! - Search provider integration
//! - Real-time result updates with background threads
//!
//! The `AppListModel` struct is the central coordinator that manages
//! all search modes, executes commands, and updates the GTK list store.

use crate::app_mode::ActiveMode;
use crate::core::config::{CommandConfig, ObsidianConfig};
use crate::launcher::DesktopApp;
use crate::model::items::SearchResultItem;
use crate::providers::dbus::{self, SearchProvider as DbusSearchProvider};
use crate::providers::{AppProvider, CalculatorProvider, CommandProvider, SearchProvider};
use gtk4::SingleSelection;
use gtk4::gio;
use gtk4::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

const DEFAULT_SEARCH_DEBOUNCE_MS: u32 = 100;
const PROVIDER_SEARCH_DEBOUNCE_MS: u32 = 120;
const PROVIDER_CLEAR_TIMEOUT_MS: u64 = 25;

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
    pub(crate) max_results: Cell<usize>,

    /// Generation counter for cancelling stale async tasks
    pub(crate) task_gen: Rc<Cell<u64>>,
    /// Obsidian configuration (if enabled)
    pub obsidian_cfg: Option<ObsidianConfig>,
    /// Current active mode for UI rendering
    pub(crate) active_mode: Rc<Cell<ActiveMode>>,
    /// Debounce timer source ID for delayed command execution
    pub(crate) command_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    /// Debounce delay in milliseconds
    pub(crate) command_debounce_ms: Cell<u32>,
    /// Debounce timer source ID for delayed search execution
    search_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    /// Debounce delay in milliseconds for default search mode
    search_debounce_ms: u32,
    /// Cached GNOME Shell search providers
    search_providers: Rc<std::cell::OnceCell<Vec<DbusSearchProvider>>>,
    /// List of search provider IDs to exclude
    search_provider_blacklist: Rc<RefCell<Vec<String>>>,
    /// List of custom script commands
    pub(crate) commands: Rc<RefCell<Vec<crate::core::config::CommandConfig>>>,
    /// Whether all special modes (colon commands) are disabled
    disable_modes: bool,
    /// Search providers for different search types
    providers: Rc<Vec<Box<dyn SearchProvider>>>,
}

impl AppListModel {
    // ── Internal API ──────────────────────────────────────────────────────────

    /// Set the current active mode
    pub(crate) fn set_active_mode(&self, mode: ActiveMode) {
        self.active_mode.set(mode);
    }

    /// Get the current active mode
    pub(crate) fn active_mode(&self) -> ActiveMode {
        self.active_mode.get()
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
        self.obsidian_cfg.as_ref()
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
        let old_max_results = self.max_results.get();

        // Update max_results
        self.max_results.set(config.max_results);

        // Update max_results on providers
        for provider in self.providers.iter() {
            provider.set_max_results(config.max_results);
        }

        // Update command debounce
        self.command_debounce_ms.set(config.command_debounce_ms);

        // Update search provider blacklist
        (*self.search_provider_blacklist.borrow_mut())
            .clone_from(&config.search_provider_blacklist);

        // Update commands
        (*self.commands.borrow_mut()).clone_from(&config.commands);

        // Repopulate if max_results changed or in CustomScript mode
        if old_max_results != config.max_results {
            let query = self.current_query.borrow().clone();
            self.populate(&query);
        } else if self.active_mode.get() == ActiveMode::CustomScript {
            use crate::command_handler::CommandHandler;
            let query = self.current_query.borrow().clone();
            let handler = CommandHandler::new(self);
            handler.handle_sh(&query);
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
    pub(crate) fn schedule_command<F>(&self, f: F)
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
        let providers = self
            .search_providers
            .get_or_init(|| dbus::discover_providers(&self.search_provider_blacklist.borrow()));

        if providers.is_empty() {
            return;
        }

        self.active_mode.set(ActiveMode::None);
        self.bump_task_gen();
        let providers_clone: Vec<DbusSearchProvider> = providers.clone();
        let max = self.max_results.get();
        let model_clone = self.clone();
        // Use shorter debounce for search providers for more responsive feel
        self.schedule_command_with_delay(PROVIDER_SEARCH_DEBOUNCE_MS, move || {
            model_clone.run_provider_search(providers_clone, query, max, clear_store);
        });
    }

    /// Increment the task generation counter and return the new value
    ///
    /// This is used to identify stale async tasks - if a task's generation
    /// doesn't match the current one, its results should be discarded.
    pub(crate) fn bump_task_gen(&self) -> u64 {
        let next_gen = self.task_gen.get() + 1;
        self.task_gen.set(next_gen);
        next_gen
    }

    /// Bump the task generation counter and schedule a command that
    /// only executes if no newer bump has occurred.
    ///
    /// This encapsulates the common bump-and-check pattern used by
    /// handlers that schedule async work which should be cancelled
    /// when the user types new input.
    pub(crate) fn bump_and_schedule<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        let generation = self.bump_task_gen();
        let model_clone = self.clone();
        self.schedule_command(move || {
            if model_clone.task_gen.get() == generation {
                f();
            }
        });
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
            // Default search: debounce via schedule_search (uses search_debounce_ms)
            self.schedule_search(move || model.populate(&query));
        }
    }

    /// Handle colon-prefixed commands by routing to appropriate handlers
    fn handle_colon_command(&self, query: &str) {
        use crate::command_handler::CommandHandler;
        let handler = CommandHandler::new(self);
        handler.handle_colon_command(query);
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
            let timeout_id = glib::timeout_add_local(
                Duration::from_millis(PROVIDER_CLEAR_TIMEOUT_MS),
                move || {
                    if clear_model.task_gen.get() == clear_gen {
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

impl CommandProvider for AppListModel {
    fn get_commands(&self, query: &str) -> Vec<CommandConfig> {
        let commands = self.commands.borrow();
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
}

#[cfg(test)]
mod tests {
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
