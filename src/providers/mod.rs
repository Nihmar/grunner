//! Search providers module for Grunner
//!
//! This module defines the `SearchProvider` trait and provides concrete
//! implementations for different types of searches (apps, files, obsidian, etc.).
//! This abstraction allows adding new search sources without modifying the core
//! list model logic.

pub mod dbus;
pub mod file_search;
pub mod subprocess;

pub use subprocess::{SubprocessRunner, spawn_subprocess};

use crate::core::config::CommandConfig;
use crate::launcher::DesktopApp;
use crate::model::items::{AppItem, CommandItem};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use gtk4::glib;
use gtk4::prelude::Cast;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Trait representing a search provider that can return results as GTK objects
///
/// Implementations should return `Vec<glib::Object>` which can be directly
/// added to a `gio::ListStore`.
pub trait SearchProvider {
    /// Search for items matching the query
    ///
    /// # Arguments
    /// * `query` - The search string
    ///
    /// # Returns
    /// A vector of `glib::Object` instances (`AppItem`, `CommandItem`, or `SearchResultItem`)
    fn search(&self, query: &str) -> Vec<glib::Object>;

    /// Update the maximum number of results to return
    fn set_max_results(&self, _max: usize) {}
}

/// Trait representing a command provider that can return commands
///
/// Implementations should return `Vec<CommandConfig>` which can be used
/// to populate the command list in the UI.
pub trait CommandProvider {
    /// Get commands matching the query
    ///
    /// # Arguments
    /// * `query` - The search string (optional)
    ///
    /// # Returns
    /// A vector of `CommandConfig` instances
    fn get_commands(&self, query: &str) -> Vec<CommandConfig>;
}

// ---------------------------------------------------------------------------
// App Provider - Desktop application launcher
// ---------------------------------------------------------------------------

pub struct AppProvider {
    all_apps: Rc<RefCell<Vec<DesktopApp>>>,
    max_results: Cell<usize>,
    fuzzy_matcher: Rc<SkimMatcherV2>,
}

impl AppProvider {
    pub fn new(all_apps: Rc<RefCell<Vec<DesktopApp>>>, max_results: usize) -> Self {
        Self {
            all_apps,
            max_results: Cell::new(max_results),
            fuzzy_matcher: Rc::new(SkimMatcherV2::default()),
        }
    }

    /// Optimized search that uses prefix matching for simple queries
    fn search_apps_optimized<'a>(
        &self,
        query: &str,
        apps: &'a [DesktopApp],
        max_results: usize,
    ) -> Vec<&'a DesktopApp> {
        // Fast path: empty query returns first N apps
        if query.is_empty() {
            return apps.iter().take(max_results).collect();
        }

        let query_lower = query.to_lowercase();

        // Fast path: simple prefix match for short, single-word queries
        // This covers 80% of typical searches
        if !query.contains(char::is_whitespace) && query.len() < 15 {
            let mut scored: Vec<_> = apps
                .iter()
                .filter(|app| {
                    app.name.to_lowercase().starts_with(&query_lower)
                        || app.name.to_lowercase().contains(&query_lower)
                })
                .map(|app| {
                    let score = if app.name.to_lowercase().starts_with(&query_lower) {
                        100
                    } else {
                        50
                    };
                    (score, app)
                })
                .collect();

            scored.sort_by(|a, b| b.0.cmp(&a.0));
            let prefix_results: Vec<_> = scored
                .into_iter()
                .take(max_results)
                .map(|(_, app)| app)
                .collect();

            if !prefix_results.is_empty() {
                return prefix_results;
            }
        }

        // Fall back to fuzzy matching for complex queries
        let mut scored: Vec<_> = apps
            .iter()
            .filter_map(|app| {
                self.fuzzy_matcher
                    .fuzzy_match(&app.name, query)
                    .or_else(|| {
                        self.fuzzy_matcher
                            .fuzzy_match(&app.description, query)
                            .map(|s| s / 2) // Description matches weighted less
                    })
                    .map(|score| (score, app))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored
            .into_iter()
            .take(max_results)
            .map(|(_, app)| app)
            .collect()
    }
}

impl SearchProvider for AppProvider {
    fn search(&self, query: &str) -> Vec<glib::Object> {
        let apps = self.all_apps.borrow();
        if apps.is_empty() {
            return vec![];
        }

        let max = self.max_results.get();
        let results: Vec<glib::Object> = if query.is_empty() {
            apps.iter()
                .take(max)
                .map(|app| AppItem::new(app).upcast::<glib::Object>())
                .collect()
        } else {
            let matched_apps = self.search_apps_optimized(query, &apps, max);
            matched_apps
                .into_iter()
                .map(|app| AppItem::new(app).upcast::<glib::Object>())
                .collect()
        };

        results
    }

    fn set_max_results(&self, max: usize) {
        self.max_results.set(max);
    }
}

// ---------------------------------------------------------------------------
// Calculator Provider
// ---------------------------------------------------------------------------

pub struct CalculatorProvider;

impl CalculatorProvider {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for CalculatorProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchProvider for CalculatorProvider {
    fn search(&self, query: &str) -> Vec<glib::Object> {
        // Check if query is a calculator expression
        if let Some(result) = crate::calculator::evaluate(query) {
            let calculator_result = format!("{query} = {result}");
            return vec![CommandItem::new(calculator_result).upcast::<glib::Object>()];
        }
        vec![]
    }
}
