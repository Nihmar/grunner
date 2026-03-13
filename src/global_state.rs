//! Centralized global state management
//!
//! This module provides a single location for all global state variables
//! using thread-local storage to avoid Sync requirements.

use std::cell::RefCell;
use std::sync::OnceLock;

use crate::config::Config;

// ─── HOME Directory ──────────────────────────────────────────────────────────

/// Cached home directory to avoid repeated environment variable lookups
static HOME_DIR: OnceLock<String> = OnceLock::new();

/// Get the home directory, caching the result for performance
pub fn get_home_dir() -> &'static str {
    HOME_DIR.get_or_init(|| {
        std::env::var_os("HOME")
            .and_then(|s| s.into_string().ok())
            .unwrap_or_else(|| ".".into())
    })
}

// ─── Tokio Runtime ──────────────────────────────────────────────────────────

/// Global Tokio runtime for async operations
static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Get or initialize the shared Tokio runtime
///
/// # Panics
/// Panics if the Tokio runtime fails to build (e.g., due to resource exhaustion).
#[must_use]
pub fn get_tokio_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_io()
            .enable_time()
            .build()
            .expect("[global_state] failed to build tokio runtime")
    })
}

// ─── Config Hot-Reload ──────────────────────────────────────────────────────

type ConfigReloader = Box<dyn Fn(&Config)>;

thread_local! {
    static CONFIG_RELOADER: RefCell<Option<ConfigReloader>> = RefCell::new(None);
}

pub fn set_config_reloader<F>(reloader: F)
where
    F: Fn(&Config) + 'static,
{
    CONFIG_RELOADER.with(|r| {
        *r.borrow_mut() = Some(Box::new(reloader));
    });
}

pub fn reload_config(config: &Config) {
    CONFIG_RELOADER.with(|r| {
        if let Some(reloader) = r.borrow().as_ref() {
            reloader(config);
        }
    });
}
