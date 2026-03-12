//! Centralized global state management
//!
//! This module provides a single location for all global state variables
//! using OnceLock to ensure thread-safe initialization.

use std::sync::OnceLock;

// ─── HOME Directory ──────────────────────────────────────────────────────────

/// Cached home directory to avoid repeated environment variable lookups
static HOME_DIR: OnceLock<String> = OnceLock::new();

/// Get the home directory, caching the result for performance
pub fn get_home_dir() -> &'static str {
    HOME_DIR.get_or_init(|| std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

// ─── Tokio Runtime ──────────────────────────────────────────────────────────

/// Global Tokio runtime for async operations
static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Get or initialize the shared Tokio runtime
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
