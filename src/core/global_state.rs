//! Centralized global state management
//!
//! This module provides a single location for global state variables
//! using `OnceLock` for lazy initialisation (HOME directory, Tokio runtime).
//!
//! Settings hot-reload callbacks have moved to `core::callbacks` (GObject signals).

use std::sync::OnceLock;

// ─── HOME Directory ──────────────────────────────────────────────────────────

static HOME_DIR: OnceLock<String> = OnceLock::new();

pub fn get_home_dir() -> &'static str {
    HOME_DIR.get_or_init(|| {
        std::env::var_os("HOME")
            .and_then(|s| s.into_string().ok())
            .unwrap_or_else(|| ".".into())
    })
}

// ─── Tokio Runtime ──────────────────────────────────────────────────────────

static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_home_dir() {
        let home = get_home_dir();
        assert!(!home.is_empty());
    }
}
