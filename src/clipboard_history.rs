use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

const DEFAULT_MAX_ITEMS: usize = 20;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct ClipboardHistory {
    items: VecDeque<String>,
    max: usize,
}

impl ClipboardHistory {
    pub fn load(max_items: Option<usize>) -> Self {
        let path = Self::path();
        let max = max_items.unwrap_or(DEFAULT_MAX_ITEMS);
        if let Ok(data) = fs::read_to_string(&path) {
            let mut hist: Self = serde_json::from_str(&data).unwrap_or_else(|_| Self {
                items: VecDeque::new(),
                max,
            });
            hist.max = max;
            hist
        } else {
            Self {
                items: VecDeque::new(),
                max,
            }
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

    pub fn push(&mut self, text: String) {
        // Avoid duplicates with the last item
        if self.items.back() == Some(&text) {
            return;
        }
        if self.items.len() >= self.max {
            self.items.pop_front();
        }
        self.items.push_back(text);
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    fn path() -> PathBuf {
        let cache = std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_default();
                PathBuf::from(home).join(".cache")
            });
        cache.join("grunner").join("clipboard_history.json")
    }
}
