use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

pub(crate) const DEFAULT_SEARCH_DEBOUNCE_MS: u32 = 100;

/// Manages debounce timers for command execution and search operations.
///
/// Provides separate scheduling for:
/// - Commands (colon commands) using `schedule_command`
/// - Search providers using `schedule_search`
pub struct DebounceScheduler {
    command_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    command_debounce_ms: Cell<u32>,
    search_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    search_debounce_ms: u32,
}

impl DebounceScheduler {
    #[must_use]
    pub fn new(command_ms: u32, search_ms: u32) -> Self {
        Self {
            command_debounce: Rc::new(RefCell::new(None)),
            command_debounce_ms: Cell::new(command_ms),
            search_debounce: Rc::new(RefCell::new(None)),
            search_debounce_ms: search_ms,
        }
    }

    #[must_use]
    pub fn command_debounce_ms(&self) -> u32 {
        self.command_debounce_ms.get()
    }

    pub fn set_command_debounce_ms(&self, ms: u32) {
        self.command_debounce_ms.set(ms);
    }

    pub fn cancel_command(&self) {
        if let Some(id) = self.command_debounce.borrow_mut().take() {
            id.remove();
        }
    }

    pub fn cancel_search(&self) {
        if let Some(id) = self.search_debounce.borrow_mut().take() {
            id.remove();
        }
    }

    pub fn schedule_command<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel_command();
        Self::schedule_with_delay(&self.command_debounce, self.command_debounce_ms.get(), f);
    }

    pub fn schedule_search<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel_search();
        Self::schedule_with_delay(&self.search_debounce, self.search_debounce_ms, f);
    }

    pub fn schedule_command_with_delay<F>(&self, delay_ms: u32, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel_command();
        Self::schedule_with_delay(&self.command_debounce, delay_ms, f);
    }

    fn schedule_with_delay<F>(slot: &Rc<RefCell<Option<glib::SourceId>>>, delay_ms: u32, f: F)
    where
        F: FnOnce() + 'static,
    {
        if let Some(id) = slot.borrow_mut().take() {
            id.remove();
        }
        let mut f_opt = Some(f);
        let slot_clone = slot.clone();
        let source_id =
            glib::timeout_add_local(Duration::from_millis(delay_ms.into()), move || {
                *slot_clone.borrow_mut() = None;
                if let Some(f) = f_opt.take() {
                    f();
                }
                glib::ControlFlow::Break
            });
        *slot.borrow_mut() = Some(source_id);
    }
}

impl Clone for DebounceScheduler {
    fn clone(&self) -> Self {
        Self {
            command_debounce: Rc::clone(&self.command_debounce),
            command_debounce_ms: Cell::new(self.command_debounce_ms.get()),
            search_debounce: Rc::clone(&self.search_debounce),
            search_debounce_ms: self.search_debounce_ms,
        }
    }
}

impl Default for DebounceScheduler {
    fn default() -> Self {
        Self::new(300, DEFAULT_SEARCH_DEBOUNCE_MS)
    }
}
