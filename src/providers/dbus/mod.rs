//! D-Bus integration for GNOME Shell search providers
//!
//! This module provides integration with GNOME Shell search providers via D-Bus.

pub mod discovery;
pub mod icons;
pub mod query;
pub mod types;

pub use discovery::discover_providers;
pub use query::{activate_result, run_search_streaming};
pub use types::{IconData, SearchProvider, SearchResult};
