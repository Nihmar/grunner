use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct LaunchHistory {
    counts: HashMap<String, u32>, // key = .desktop file path
}

impl LaunchHistory {
    pub fn load() -> Self {
        let path = Self::path();
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, data);
        }
    }

    pub fn record_launch(&mut self, app_path: &str) {
        *self.counts.entry(app_path.to_string()).or_insert(0) += 1;
    }

    pub fn get_count(&self, app_path: &str) -> u32 {
        self.counts.get(app_path).copied().unwrap_or(0)
    }

    fn path() -> PathBuf {
        let cache = std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_default();
                PathBuf::from(home).join(".cache")
            });
        cache.join("grunner").join("launch_history.json")
    }
}
