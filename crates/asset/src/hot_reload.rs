use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event};
use parking_lot::RwLock;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// Event emitted when a watched file changes.
#[derive(Debug, Clone)]
pub struct FileEvent {
    pub path: PathBuf,
    pub kind: FileChangeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeKind {
    Created,
    Modified,
    Removed,
}

/// Manages file system watching for hot-reload during development.
pub struct HotReloader {
    watcher: Option<RecommendedWatcher>,
    rx: UnboundedReceiver<FileEvent>,
    watched: Arc<RwLock<HashMap<PathBuf, SystemTime>>>,
}

impl HotReloader {
    pub fn new() -> Self {
        let (tx, rx): (UnboundedSender<FileEvent>, UnboundedReceiver<FileEvent>) = tokio::sync::mpsc::unbounded_channel();
        
        let watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let kind = match event.kind {
                    notify::EventKind::Create(_) => FileChangeKind::Created,
                    notify::EventKind::Modify(_) => FileChangeKind::Modified,
                    notify::EventKind::Remove(_) => FileChangeKind::Removed,
                    _ => return,
                };
                for path in event.paths {
                    let _ = tx.send(FileEvent { path, kind });
                }
            }
        }).ok();

        Self { watcher, rx, watched: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Watch a file path for changes.
    pub fn watch(&mut self, path: impl AsRef<Path>) -> notify::Result<()> {
        let path = path.as_ref().to_path_buf();
        if let Some(ref mut watcher) = self.watcher {
            watcher.watch(&path, RecursiveMode::NonRecursive)?;
        }
        let mod_time = path.metadata().ok().and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH);
        self.watched.write().insert(path, mod_time);
        Ok(())
    }

    /// Unwatch a file path.
    pub fn unwatch(&mut self, path: impl AsRef<Path>) -> notify::Result<()> {
        if let Some(ref mut watcher) = self.watcher {
            watcher.unwatch(path.as_ref())?;
        }
        Ok(())
    }

    /// Poll for file system events.
    pub fn poll(&mut self) -> impl Iterator<Item = FileEvent> + '_ {
        std::iter::from_fn(move || self.rx.try_recv().ok()).collect::<Vec<_>>().into_iter()
    }
}

impl Default for HotReloader {
    fn default() -> Self {
        Self::new()
    }
}