//! Hot-reload of `.rhai` scripts without restarting the engine.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Tracks file modification times for hot reload.
#[derive(Debug, Default)]
pub struct HotReloadWatcher {
    pub tracked: HashMap<PathBuf, SystemTime>,
}

impl HotReloadWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start tracking a script file.
    pub fn track(&mut self, path: PathBuf) {
        let modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        self.tracked.insert(path, modified);
    }

    /// Check all tracked files and return paths that have changed.
    pub fn check(&mut self) -> Vec<PathBuf> {
        let mut changed = Vec::new();
        for (path, last_modified) in &mut self.tracked {
            let current = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if current > *last_modified {
                *last_modified = current;
                changed.push(path.clone());
            }
        }
        changed
    }

    pub fn untrack(&mut self, path: &PathBuf) {
        self.tracked.remove(path);
    }
}
