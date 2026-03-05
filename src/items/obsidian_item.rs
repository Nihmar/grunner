//! GTK Object wrapper for Obsidian action items
//!
//! This module provides `ObsidianActionItem`, a GTK object that represents
//! Obsidian-specific actions (like opening the vault, creating new notes)
//! for use in GTK list models and UI components. It implements the necessary
//! GTK object subclassing to make Obsidian action data accessible to GTK's
//! property system and list views.

use glib::Object;
use glib::subclass::prelude::*;
use std::cell::RefCell;

/// Enum representing different Obsidian actions that can be performed
///
/// These actions correspond to common Obsidian operations that Grunner
/// can trigger via the Obsidian URI scheme or file system operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObsidianAction {
    /// Open the entire Obsidian vault in the Obsidian app
    #[default]
    OpenVault,
    /// Create a new note with optional text content
    NewNote,
    /// Open or append to today's daily note
    DailyNote,
    /// Append text to the configured quick note file
    QuickNote,
}

/// Internal implementation module for GTK object subclassing
///
/// GTK requires object implementations to be separated into an `imp` module
/// for encapsulation and proper object lifecycle management.
mod imp {
    use super::*;

    /// Internal data structure holding Obsidian action metadata
    ///
    /// This struct stores the action type and optional argument text
    /// that will be used when performing the Obsidian action.
    #[derive(Default)]
    pub struct ObsidianActionItem {
        /// The type of Obsidian action to perform, wrapped in RefCell for interior mutability
        pub action: RefCell<ObsidianAction>,
        /// Optional argument text (e.g., note content), wrapped in RefCell for interior mutability
        pub arg: RefCell<Option<String>>,
    }

    /// GTK object subclass implementation
    ///
    /// This macro-based implementation registers the object type with GTK
    /// and defines its behavior. The `ObjectImpl` trait is implemented
    /// with default behavior since no custom object methods are needed.
    #[glib::object_subclass]
    impl ObjectSubclass for ObsidianActionItem {
        /// Unique type name for GTK's type system
        const NAME: &'static str = "GrunnerObsidianActionItem";
        /// Associated parent type (the public ObsidianActionItem struct)
        type Type = super::ObsidianActionItem;
    }

    /// Default implementation of GTK object methods
    ///
    /// No custom object behavior is needed beyond data storage,
    /// so we use the default implementation.
    impl ObjectImpl for ObsidianActionItem {}
}

// Public GTK object wrapper for Obsidian action items
//
// This is the public-facing type that UI code interacts with.
// It wraps the internal GTK object implementation and provides
// a clean API for creating and accessing Obsidian action data.
//
// The `glib::wrapper!` macro generates the necessary boilerplate
// to expose this as a proper GTK object.
glib::wrapper! {
    pub struct ObsidianActionItem(ObjectSubclass<imp::ObsidianActionItem>);
}

impl ObsidianActionItem {
    /// Create a new ObsidianActionItem from an action and optional argument
    ///
    /// # Arguments
    /// * `action` - The ObsidianAction variant to perform
    /// * `arg` - Optional text argument for the action (e.g., note content)
    ///
    /// # Returns
    /// A new `ObsidianActionItem` GTK object populated with the action data.
    pub fn new(action: ObsidianAction, arg: Option<String>) -> Self {
        // Create a new GTK object instance
        let obj: Self = Object::new();
        // Initialize the internal data with the action and argument
        *obj.imp().action.borrow_mut() = action;
        *obj.imp().arg.borrow_mut() = arg;
        obj
    }

    /// Get the Obsidian action stored in this item
    ///
    /// # Returns
    /// The `ObsidianAction` variant that this item represents.
    pub fn action(&self) -> ObsidianAction {
        *self.imp().action.borrow()
    }

    /// Get the optional argument text stored in this item
    ///
    /// # Returns
    /// `Some(String)` if the action has an associated argument,
    /// `None` if no argument was provided.
    ///
    /// # Usage
    /// For `NewNote`, this is the initial note content.
    /// For `DailyNote` and `QuickNote`, this is text to append.
    /// For `OpenVault`, this is typically `None`.
    pub fn arg(&self) -> Option<String> {
        self.imp().arg.borrow().clone()
    }
}
