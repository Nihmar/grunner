use crate::app_mode::ActiveMode;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Manages search state: current query and task generation for cancellation.
///
/// Task generation allows stale async operations to be detected and discarded
/// when the user types new input before previous searches complete.
#[derive(Clone)]
pub struct SearchState {
    current_query: Rc<RefCell<String>>,
    task_gen: Rc<Cell<u64>>,
    active_mode: Rc<Cell<ActiveMode>>,
}

impl SearchState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_query: Rc::new(RefCell::new(String::new())),
            task_gen: Rc::new(Cell::new(0)),
            active_mode: Rc::new(Cell::new(ActiveMode::None)),
        }
    }

    #[must_use]
    pub fn current_query(&self) -> String {
        self.current_query.borrow().clone()
    }

    pub fn set_query(&self, query: &str) {
        *self.current_query.borrow_mut() = query.to_string();
    }

    #[must_use]
    pub fn active_mode(&self) -> ActiveMode {
        self.active_mode.get()
    }

    pub fn set_active_mode(&self, mode: ActiveMode) {
        self.active_mode.set(mode);
    }

    #[must_use]
    pub fn bump_task_gen(&self) -> u64 {
        let next = self.task_gen.get() + 1;
        self.task_gen.set(next);
        next
    }

    #[must_use]
    pub fn task_gen(&self) -> u64 {
        self.task_gen.get()
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}
