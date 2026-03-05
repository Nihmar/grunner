//! GObject wrapper types for GTK list models.
//!
//! Each sub-module provides a thin GObject subclass used to store
//! domain data inside a `gtk4::ListStore`.  All public types are
//! re-exported here so call sites can continue to write
//! `use crate::items::AppItem` (or whatever they imported before)
//! without any changes.

mod app_item;
mod cmd_item;
mod obsidian_item;
mod search_result_item;

pub use app_item::AppItem;
pub use cmd_item::CommandItem;
pub use obsidian_item::{ObsidianAction, ObsidianActionItem};
pub use search_result_item::SearchResultItem;
