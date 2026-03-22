use crate::core::config::{CommandConfig, ObsidianConfig};
use crate::launcher::DesktopApp;
use crate::providers::{AppProvider, CalculatorProvider, SearchProvider};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Holds configuration settings for the search model.
///
/// Contains values that can be updated via `apply_config` without recreating the model.
#[derive(Clone)]
pub struct ModelConfig {
    pub max_results: Cell<usize>,
    pub obsidian_cfg: Option<ObsidianConfig>,
    pub commands: Rc<RefCell<Vec<CommandConfig>>>,
    pub blacklist: Rc<RefCell<Vec<String>>>,
    pub disable_modes: Cell<bool>,
    pub providers: Rc<Vec<Box<dyn SearchProvider>>>,
}

impl ModelConfig {
    pub fn new(
        max_results: usize,
        obsidian_cfg: Option<ObsidianConfig>,
        blacklist: Vec<String>,
        commands: Vec<CommandConfig>,
        disable_modes: bool,
        all_apps: Rc<RefCell<Vec<DesktopApp>>>,
    ) -> Self {
        let providers = Rc::new(vec![
            Box::new(AppProvider::new(all_apps, max_results)) as Box<dyn SearchProvider>,
            Box::new(CalculatorProvider::new()) as Box<dyn SearchProvider>,
        ]);

        Self {
            max_results: Cell::new(max_results),
            obsidian_cfg,
            commands: Rc::new(RefCell::new(commands)),
            blacklist: Rc::new(RefCell::new(blacklist)),
            disable_modes: Cell::new(disable_modes),
            providers,
        }
    }

    pub fn apply_config(&self, config: &crate::core::config::Config) {
        self.max_results.set(config.max_results);
        self.disable_modes.set(config.disable_modes);

        for provider in self.providers.iter() {
            provider.set_max_results(config.max_results);
        }

        (*self.blacklist.borrow_mut()).clone_from(&config.search_provider_blacklist);
        (*self.commands.borrow_mut()).clone_from(&config.commands);
    }
}
