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

use crate::app_item::AppItem;
use crate::cmd_item::CommandItem;
use crate::config::ObsidianConfig;
use crate::launcher::DesktopApp;
use crate::search_provider::{self, SearchProvider};
use crate::search_result_item::SearchResultItem;
use crate::utils::expand_home;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use gtk4::gio;
use gtk4::prelude::Cast;
use gtk4::prelude::*;
use gtk4::{ListItem, SignalListItemFactory, SingleSelection};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

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

/// Tracks the current active search mode for UI rendering
///
/// This enum determines how items in the list should be displayed
/// and what icons/descriptions should be shown for each result type.
#[derive(Clone, Copy, Default, PartialEq)]
enum ActiveMode {
    /// Default mode - no special rendering
    #[default]
    None,
    /// GNOME Shell search provider results
    SearchProvider,
    /// Obsidian action mode (vault open, new note, etc.)
    ObsidianAction,
    /// Obsidian file search results
    ObsidianFile,
    /// Obsidian grep (ripgrep) search results
    ObsidianGrep,
}

/// Set description label text with visibility handling
///
/// Shows the label only if text is non-empty, hiding it completely
/// when there's no description to avoid empty space in the UI.
fn set_desc(label: &gtk4::Label, text: &str) {
    let visible = !text.is_empty();
    label.set_visible(visible);
    label.set_text(if visible { text } else { "" });
}

/// Convert absolute file path to vault-relative path for display
///
/// Strips the vault path prefix from absolute paths to show cleaner
/// relative paths in the UI when displaying Obsidian search results.
fn relative_to_vault<'a>(path: &'a str, vault: &Option<String>) -> &'a str {
    vault
        .as_deref()
        .and_then(|v| path.strip_prefix(v))
        .map(|s| s.trim_start_matches('/'))
        .unwrap_or(path)
}

/// Bind application item data to UI widgets
///
/// Sets up the icon, name, and description for a desktop application
/// entry in the list view. Handles both themed icons and file-based icons.
fn bind_app_item(
    item: &AppItem,
    image: &gtk4::Image,
    name_label: &gtk4::Label,
    desc_label: &gtk4::Label,
) {
    let icon = item.icon();
    if icon.is_empty() {
        // Default executable icon for apps without specified icon
        image.set_icon_name(Some("application-x-executable"));
    } else if icon.starts_with('/') {
        // Absolute path to icon file
        image.set_from_file(Some(&icon));
    } else {
        // Themed icon name
        image.set_icon_name(Some(&icon));
    }
    name_label.set_text(&item.name());
    set_desc(desc_label, &item.description());
}

/// Bind command item data to UI widgets based on active mode
///
/// This function handles the complex logic of displaying different types
/// of command results based on the current search mode:
/// - Obsidian grep results with file:line formatting
/// - File paths with appropriate icons based on file type
/// - Obsidian file search results with vault-relative paths
/// - Generic command output with search icon
fn bind_command_item(
    item: &CommandItem,
    image: &gtk4::Image,
    name_label: &gtk4::Label,
    desc_label: &gtk4::Label,
    mode: ActiveMode,
    vault_path: &Option<String>,
    obsidian_icon: &str,
) {
    let line = item.line();

    // Handle Obsidian grep results (file:line:content format)
    if mode == ActiveMode::ObsidianGrep {
        image.set_icon_name(Some(obsidian_icon));
        if let Some((file_path, rest)) = line.split_once(':') {
            // Show vault-relative path and grep match content
            name_label.set_text(relative_to_vault(file_path, vault_path));
            set_desc(desc_label, rest);
        } else {
            name_label.set_text(&line);
            set_desc(desc_label, "");
        }
        return;
    }

    // Handle absolute file paths
    if line.starts_with('/') {
        if !line.contains(':') {
            // Plain file path (no line number)
            if mode == ActiveMode::ObsidianFile {
                // Obsidian file search - use Obsidian icon
                image.set_icon_name(Some(obsidian_icon));
                let filename = std::path::Path::new(&line)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&line);
                name_label.set_text(filename);
                let relative = relative_to_vault(&line, vault_path);
                let parent = std::path::Path::new(relative)
                    .parent()
                    .and_then(|p| p.to_str())
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        std::path::Path::new(&line)
                            .parent()
                            .and_then(|p| p.to_str())
                    });
                set_desc(desc_label, parent.unwrap_or(""));
            } else {
                // Regular file search - use file type icon
                let (ctype, _) = gio::content_type_guess(Some(line.as_str()), None::<&[u8]>);
                image.set_from_gicon(&gio::content_type_get_icon(&ctype));
                let filename = std::path::Path::new(&line)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&line);
                name_label.set_text(filename);
                let parent = std::path::Path::new(&line)
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("");
                set_desc(desc_label, parent);
            }
            return;
        }

        // Absolute path with colon – grep output from :fg command
        if let Some((file_path, rest)) = line.split_once(':') {
            let (ctype, _) = gio::content_type_guess(Some(file_path), None::<&[u8]>);
            image.set_from_gicon(&gio::content_type_get_icon(&ctype));
            let filename = std::path::Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path);
            name_label.set_text(filename);
            set_desc(desc_label, rest);
            return;
        }
    }

    // Fallback for any other lines (generic command output)
    image.set_icon_name(Some("system-search"));
    name_label.set_text(&line);
    set_desc(desc_label, "");
}

/// Bind search provider result item data to UI widgets
///
/// Handles GNOME Shell search provider results with multiple icon sources:
/// 1. File-based icons (absolute paths)
/// 2. Themed icons (icon names)
/// 3. Application icons (from provider metadata)
/// 4. Fallback search icon
fn bind_search_result_item(
    item: &SearchResultItem,
    image: &gtk4::Image,
    name_label: &gtk4::Label,
    desc_label: &gtk4::Label,
) {
    let icon_file = item.icon_file();
    let icon_themed = item.icon_themed();
    let app_icon = item.app_icon_name();
    if !icon_file.is_empty() {
        image.set_from_file(Some(&icon_file));
    } else if !icon_themed.is_empty() {
        image.set_icon_name(Some(&icon_themed));
    } else if !app_icon.is_empty() {
        image.set_icon_name(Some(&app_icon));
    } else {
        image.set_icon_name(Some("system-search"));
    }
    name_label.set_text(&item.name());
    set_desc(desc_label, &item.description());
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
    rx: std::sync::mpsc::Receiver<Vec<search_provider::SearchResult>>,
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
                                Some(search_provider::IconData::Themed(n)) => (n, String::new()),
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
                                this.terms.clone(),
                            )
                            .upcast::<glib::Object>()
                        })
                        .collect();

                    // Clear store only on first batch
                    if !this.first_batch.get() {
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
    max_results: usize,

    /// Custom shell commands for colon modes
    commands: Rc<HashMap<String, String>>,
    /// Generation counter for cancelling stale async tasks
    task_gen: Rc<Cell<u64>>,
    /// Obsidian configuration (if enabled)
    pub obsidian_cfg: Option<ObsidianConfig>,
    /// Current active mode for UI rendering
    active_mode: Rc<Cell<ActiveMode>>,
    /// Debounce timer source ID for delayed command execution
    command_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    /// Debounce delay in milliseconds
    command_debounce_ms: u32,
    /// Fuzzy matcher for application search
    fuzzy_matcher: Rc<SkimMatcherV2>,
    /// Cached GNOME Shell search providers
    search_providers: Rc<std::cell::OnceCell<Vec<SearchProvider>>>,
    /// List of search provider IDs to exclude
    search_provider_blacklist: Vec<String>,
}

impl AppListModel {
    /// Create a new AppListModel with the given configuration
    ///
    /// # Arguments
    /// * `max_results` - Maximum number of search results to display
    /// * `commands` - Custom shell commands for colon modes
    /// * `obsidian_cfg` - Optional Obsidian configuration
    /// * `command_debounce_ms` - Debounce delay for command execution
    /// * `search_provider_blacklist` - List of provider IDs to exclude
    pub fn new(
        max_results: usize,
        commands: HashMap<String, String>,
        obsidian_cfg: Option<ObsidianConfig>,
        command_debounce_ms: u32,
        search_provider_blacklist: Vec<String>,
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
            commands: Rc::new(commands),
            task_gen: Rc::new(Cell::new(0)),
            obsidian_cfg,
            active_mode: Rc::new(Cell::new(ActiveMode::None)),
            command_debounce: Rc::new(RefCell::new(None)),
            command_debounce_ms,
            fuzzy_matcher: Rc::new(SkimMatcherV2::default()),
            search_providers: Rc::new(std::cell::OnceCell::new()),
            search_provider_blacklist,
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

    /// Cancel any pending command debounce timer
    ///
    /// Used when the user types new input before a delayed command executes.
    fn cancel_debounce(&self) {
        if let Some(source_id) = self.command_debounce.borrow_mut().take() {
            let _ = source_id.remove();
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

    /// Schedule a command to run with the configured default debounce delay
    fn schedule_command<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.schedule_command_with_delay(self.command_debounce_ms, f);
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

        // Handle colon-prefixed commands
        if query.starts_with(':') {
            self.handle_colon_command(query);
            return;
        }

        // Regular application search
        self.store.remove_all();
        self.bump_task_gen();

        let apps = self.all_apps.borrow();
        if query.is_empty() {
            // Show all applications when query is empty
            for app in apps.iter() {
                self.store.append(&AppItem::new(app));
            }
        } else {
            // Perform fuzzy search on application names and descriptions
            let mut results: Vec<(i64, &DesktopApp)> = apps
                .iter()
                .filter_map(|app| {
                    let name_score = self.fuzzy_matcher.fuzzy_match(&app.name, query);
                    let desc_score = if !app.description.is_empty() {
                        self.fuzzy_matcher
                            .fuzzy_match(&app.description, query)
                            .map(|s| s / 2) // Description matches weighted less
                    } else {
                        None
                    };
                    let score = match (name_score, desc_score) {
                        (None, None) => return None, // No match at all
                        (a, b) => a.unwrap_or(i64::MIN).max(b.unwrap_or(i64::MIN)),
                    };
                    Some((score, app))
                })
                .collect();

            // Sort by score (highest first) and limit results
            results.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            results.truncate(self.max_results);

            // Add matched applications to the store
            for (_, app) in results {
                self.store.append(&AppItem::new(app));
            }
        }

        // Auto-select first item if we have results
        if self.store.n_items() > 0 {
            self.selection.set_selected(0);
        }
    }

    /// Handle colon-prefixed commands by routing to appropriate handlers
    fn handle_colon_command(&self, query: &str) {
        let (cmd_part, arg) = parse_colon_command(query);

        match cmd_part {
            "s" => self.handle_search_provider(arg),
            "ob" | "obg" => self.handle_obsidian(cmd_part, arg),
            cmd_name => self.handle_custom_command(cmd_name, arg),
        }
    }

    /// Handle search provider mode triggered by `:s` command
    fn handle_search_provider(&self, arg: &str) {
        if arg.is_empty() {
            self.clear_store();
            return;
        }

        // Discover providers (cached after first use)
        let providers = self
            .search_providers
            .get_or_init(|| search_provider::discover_providers(&self.search_provider_blacklist));

        if providers.is_empty() {
            self.show_error_item("No GNOME Shell search providers found");
            return;
        }

        self.active_mode.set(ActiveMode::SearchProvider);
        self.bump_task_gen();
        let providers_clone: Vec<SearchProvider> = providers.to_vec();
        let arg = arg.to_string();
        let max = self.max_results;
        let model_clone = self.clone();
        // Use shorter debounce for search providers for more responsive feel
        self.schedule_command_with_delay(120, move || {
            model_clone.run_provider_search(providers_clone, arg, max);
        });
    }

    /// Validate the Obsidian vault path from configuration
    ///
    /// Returns `Some(PathBuf)` if vault is configured and exists,
    /// otherwise shows an error and returns `None`.
    fn validated_vault_path(&self) -> Option<PathBuf> {
        let obs_cfg = match &self.obsidian_cfg {
            Some(c) => c.clone(),
            None => {
                self.show_error_item("Obsidian not configured - edit config");
                return None;
            }
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
                // :obg with argument - ripgrep search in vault
                let arg = arg.to_string();
                let model_clone = self.clone();
                (
                    ActiveMode::ObsidianGrep,
                    Box::new(move || model_clone.run_rg_in_vault(PathBuf::from(vault_str), &arg)),
                )
            }
            _ => unreachable!(),
        };

        self.active_mode.set(mode);
        self.bump_task_gen();
        self.schedule_command(runner);
    }

    /// Handle custom colon commands defined in configuration
    fn handle_custom_command(&self, cmd_name: &str, arg: &str) {
        let Some(template) = self.commands.get(cmd_name) else {
            return; // Unknown command - silently ignore
        };

        if arg.is_empty() {
            self.clear_store();
            return;
        };

        self.bump_task_gen();
        let template = template.clone();
        let arg = arg.to_string();
        let cmd_name = cmd_name.to_string();
        let model_clone = self.clone();
        self.schedule_command(move || {
            model_clone.run_command(&cmd_name, &template, &arg);
        });
    }

    /// Run a subprocess command and collect its output in a background thread
    ///
    /// The command output is sent back to the main thread via a channel,
    /// then processed by a `SubprocessPoller` to update the UI.
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
    fn run_provider_search(&self, providers: Vec<SearchProvider>, query: String, max: usize) {
        let generation = self.task_gen.get();
        let model_clone = self.clone();
        let terms: Vec<String> = query.split_whitespace().map(String::from).collect();

        // Set up a short timeout to clear old results and show "searching" state
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

        // Channel for streaming results from background thread
        let (tx, rx) = std::sync::mpsc::channel::<Vec<search_provider::SearchResult>>();
        std::thread::spawn(move || {
            search_provider::run_search_streaming(&providers, &query, max, tx);
        });

        let poller = ProviderSearchPoller {
            rx,
            model: model_clone,
            generation,
            terms,
            clear_timeout,
            first_batch: Rc::new(Cell::new(false)),
        };
        glib::idle_add_local_once(move || poller.poll());
    }

    /// Execute a custom shell command template with argument substitution
    fn run_command(&self, _cmd_name: &str, template: &str, argument: &str) {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(template).arg("--").arg(argument);
        self.run_subprocess(cmd);
    }

    /// Run `find` command to search for files in Obsidian vault
    fn run_find_in_vault(&self, vault_path: PathBuf, pattern: &str) {
        let mut cmd = std::process::Command::new("find");
        cmd.arg(&vault_path)
            .arg("-type")
            .arg("f")
            .arg("-iname")
            .arg(format!("*{}*", pattern));
        self.run_subprocess(cmd);
    }

    /// Run `rg` (ripgrep) command to search file contents in Obsidian vault
    fn run_rg_in_vault(&self, vault_path: PathBuf, pattern: &str) {
        let mut cmd = std::process::Command::new("rg");
        cmd.arg("--with-filename")
            .arg("--line-number")
            .arg("--no-heading")
            .arg("--color=never")
            .arg(pattern)
            .arg(&vault_path);
        self.run_subprocess(cmd);
    }

    /// Create a GTK SignalListItemFactory for rendering list items
    ///
    /// This factory sets up the UI template for each list item and
    /// binds the appropriate data based on the item type and active mode.
    pub fn create_factory(&self) -> SignalListItemFactory {
        let factory = SignalListItemFactory::new();

        let active_mode = self.active_mode.clone();
        let vault_path = self
            .obsidian_cfg
            .as_ref()
            .map(|cfg| expand_home(&cfg.vault).to_string_lossy().into_owned());

        // Try to find Obsidian icon from various possible names
        let obsidian_icon = ["obsidian", "md.obsidian.Obsidian", "Obsidian"]
            .iter()
            .map(|id| crate::search_provider::resolve_app_icon(id))
            .find(|s| !s.is_empty())
            .unwrap_or_else(|| "text-x-markdown".to_string());

        // Setup: Create UI widgets for each list item
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

        // Bind: Update widget content when item is displayed
        factory.connect_bind(move |_, list_item| {
            let list_item = list_item.downcast_ref::<ListItem>().unwrap();
            let obj = match list_item.item() {
                Some(o) => o,
                None => return,
            };

            // Extract widgets from the list item
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

            // Bind data based on item type
            if let Some(app) = obj.downcast_ref::<AppItem>() {
                bind_app_item(app, &image, &name_label, &desc_label);
            } else if let Some(cmd) = obj.downcast_ref::<CommandItem>() {
                bind_command_item(
                    cmd,
                    &image,
                    &name_label,
                    &desc_label,
                    active_mode.get(),
                    &vault_path,
                    &obsidian_icon,
                );
            } else if let Some(sr) = obj.downcast_ref::<SearchResultItem>() {
                bind_search_result_item(sr, &image, &name_label, &desc_label);
            } else {
                // Unknown item type - show placeholder
                name_label.set_text("?");
                set_desc(&desc_label, "");
            }
        });

        factory
    }
}
