//! Subprocess execution infrastructure for providers
//!
//! This module provides a generic subprocess runner that spawns background
//! threads to execute commands and delivers results through channels.
//! It supports generation tracking to cancel stale tasks.

use crate::model::list_model::AppListModel;
use gtk4::glib;

/// Unified subprocess execution handler
///
/// This struct encapsulates the common pattern of spawning a background thread,
/// sending results through a channel, and polling for results in the main thread.
/// It supports different result types and generation tracking to cancel stale tasks.
pub struct SubprocessRunner<R> {
    /// Channel receiver for results
    rx: std::sync::mpsc::Receiver<R>,
    /// Reference to the main list model for UI updates
    model: AppListModel,
    /// Generation ID to prevent stale updates after new searches
    generation: u64,
    /// Callback to process results and update the UI
    #[allow(clippy::type_complexity)]
    processor: Box<dyn Fn(&AppListModel, u64, R) + 'static>,
}

impl<R: 'static> SubprocessRunner<R> {
    /// Create a new subprocess runner
    ///
    /// # Arguments
    /// * `rx` - Channel receiver for results
    /// * `model` - Reference to the `AppListModel` for UI updates
    /// * `generation` - Generation ID to track stale tasks
    /// * `processor` - Callback to process results and update UI
    pub fn new<F>(
        rx: std::sync::mpsc::Receiver<R>,
        model: AppListModel,
        generation: u64,
        processor: F,
    ) -> Self
    where
        F: Fn(&AppListModel, u64, R) + 'static,
    {
        Self {
            rx,
            model,
            generation,
            processor: Box::new(processor),
        }
    }

    /// Poll for subprocess results and update UI when ready
    ///
    /// This method checks for available output from the background thread
    /// and updates the list store if the generation still matches.
    /// If no data is ready yet, it schedules itself to run again on idle.
    pub fn poll(self) {
        match self.rx.try_recv() {
            Ok(results) => {
                if self.model.task_gen.get() == self.generation {
                    (self.processor)(&self.model, self.generation, results);
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                glib::idle_add_local_once(move || self.poll());
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // Thread finished without sending data
            }
        }
    }
}

/// Spawn a subprocess with the given closure
///
/// This static method creates a background thread that executes the provided
/// command, collects results, and sends them through the channel.
///
/// # Arguments
/// * `cmd_fn` - Closure that creates and configures the Command
/// * `max_results` - Maximum number of results to collect
/// * `tx` - Channel sender for results
pub fn spawn_subprocess<F>(cmd_fn: F, max_results: usize, tx: std::sync::mpsc::Sender<Vec<String>>)
where
    F: FnOnce() -> std::process::Command + Send + 'static,
{
    std::thread::spawn(move || {
        let lines = cmd_fn()
            .output()
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .take(max_results)
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let _ = tx.send(lines);
    });
}
