//! Settings hot-reload signals for Grunner
//!
//! Provides `AppCallbacks`, a lightweight `GObject` that carries three
//! notification signals emitted by the settings window after a
//! successful save:
//!
//! - `config-changed` — model configuration changed
//! - `theme-changed`  — theme / appearance changed
//! - `window-resized` — window dimensions changed
//!
//! Handlers are connected once in `ui::window::build_ui` and receive
//! no parameters — they re-read whatever state they need from
//! `config::load()`.

use glib::prelude::ObjectExt;

mod imp {
    use glib::subclass::Signal;
    use glib::subclass::prelude::*;
    use std::sync::OnceLock;

    #[derive(Default)]
    pub struct AppCallbacks;

    #[glib::object_subclass]
    impl ObjectSubclass for AppCallbacks {
        const NAME: &'static str = "GrunnerAppCallbacks";
        type Type = super::AppCallbacks;
    }

    impl ObjectImpl for AppCallbacks {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("config-changed").build(),
                    Signal::builder("theme-changed").build(),
                    Signal::builder("window-resized").build(),
                ]
            })
        }
    }
}

glib::wrapper! {
    pub struct AppCallbacks(ObjectSubclass<imp::AppCallbacks>);
}

impl AppCallbacks {
    #[must_use]
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn emit_config_changed(&self) {
        self.emit_by_name::<()>("config-changed", &[]);
    }

    pub fn emit_theme_changed(&self) {
        self.emit_by_name::<()>("theme-changed", &[]);
    }

    pub fn emit_window_resized(&self) {
        self.emit_by_name::<()>("window-resized", &[]);
    }

    /// # Panics
    ///
    /// Panics if the signal value cannot be downcast to `Self`.
    pub fn connect_config_changed<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("config-changed", false, move |values| {
            let obj = values[0].get::<Self>().expect("wrong type in signal");
            f(&obj);
            None
        })
    }

    /// # Panics
    ///
    /// Panics if the signal value cannot be downcast to `Self`.
    pub fn connect_theme_changed<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("theme-changed", false, move |values| {
            let obj = values[0].get::<Self>().expect("wrong type in signal");
            f(&obj);
            None
        })
    }

    /// # Panics
    ///
    /// Panics if the signal value cannot be downcast to `Self`.
    pub fn connect_window_resized<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("window-resized", false, move |values| {
            let obj = values[0].get::<Self>().expect("wrong type in signal");
            f(&obj);
            None
        })
    }
}

impl Default for AppCallbacks {
    fn default() -> Self {
        Self::new()
    }
}
