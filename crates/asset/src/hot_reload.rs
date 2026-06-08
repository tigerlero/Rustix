use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event};
use parking_lot::RwLock;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::importer::ReloadRegistry;
use crate::server::AssetServer;

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

// ── Hot Reload Service ──

/// Bridges file system watching (`HotReloader`), asset reimport (`ReloadRegistry`),
/// and the `AssetServer` so that changed source files are automatically reimported
/// and existing handles remain valid (generation is bumped).
pub struct HotReloadService {
    pub reloader: HotReloader,
    pub registry: ReloadRegistry,
}

impl HotReloadService {
    pub fn new() -> Self {
        Self {
            reloader: HotReloader::new(),
            registry: ReloadRegistry::new(),
        }
    }

    /// Watch a file for changes and register its extension for reloading.
    pub fn watch(&mut self, path: impl AsRef<Path>) -> notify::Result<()> {
        self.reloader.watch(&path)
    }

    /// Poll file system events, reimport changed files, and replace assets in the server.
    ///
    /// Call this once per frame (or on a timer) during development.
    /// Returns the number of successfully reloaded assets.
    pub fn poll_and_reload(&mut self, server: &mut AssetServer) -> usize {
        let events: Vec<FileEvent> = self.reloader.poll().collect();
        let mut reloaded = 0;
        for event in events {
            if event.kind == FileChangeKind::Removed {
                continue;
            }
            let path = &event.path;
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            // Look up existing handle
            let handle = match server.get_by_path(path) {
                Some(h) => h,
                None => {
                    tracing::debug!("hot-reload: no tracked asset for {}", path.display());
                    continue;
                }
            };

            // Read file bytes
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("hot-reload: failed to read {}: {}", path.display(), e);
                    continue;
                }
            };

            // Reimport via registry
            let hint = path.to_str();
            let maybe_asset = self.registry.reload(ext, &bytes, hint);
            let asset = match maybe_asset {
                Some(Ok(a)) => a,
                Some(Err(e)) => {
                    tracing::warn!("hot-reload: failed to reimport {}: {}", path.display(), e);
                    continue;
                }
                None => {
                    tracing::debug!("hot-reload: no reload function for extension '{}' ({})", ext, path.display());
                    continue;
                }
            };

            // Replace in server (generation bump)
            match server.replace_untyped(handle, asset) {
                Some(new_handle) => {
                    tracing::info!("hot-reload: reloaded {} (handle {} -> {})", path.display(), handle.index, new_handle.index);
                    reloaded += 1;
                }
                None => {
                    tracing::warn!("hot-reload: stale handle for {}, reload skipped", path.display());
                }
            }
        }
        reloaded
    }
}

impl Default for HotReloadService {
    fn default() -> Self {
        Self::new()
    }
}