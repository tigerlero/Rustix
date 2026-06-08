//! Priority-ordered asset streaming: load/unload based on game state.
//!
//! `StreamingSystem` maintains a max-heap of load requests and a registry
//! of currently-loaded assets with their priority levels.  Each tick it
//! processes the highest-priority requests and, if a memory budget is
//! exceeded, unloads the lowest-priority assets that are no longer
//! referenced by active game systems.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::handle::{Asset, Handle, UntypedHandle};
use crate::server::AssetServer;

/// Priority levels for streaming.  Higher values = more urgent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StreamingPriority {
    /// Critical assets needed immediately (UI, player model, main menu).
    Critical = 5,
    /// Nearby visible objects, weapons, vehicles.
    High = 4,
    /// Medium-distance LOD0 / gameplay-critical props.
    Medium = 3,
    /// Far-distance LOD1 / background scenery.
    Low = 2,
    /// Preload for an upcoming level or area.
    Background = 1,
}

/// A request to load or unload an asset.
#[derive(Debug, Clone)]
pub struct StreamingRequest {
    pub path: PathBuf,
    pub priority: StreamingPriority,
    pub kind: RequestKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    Load,
    Unload,
}

/// Entry tracking a loaded asset's streaming metadata.
#[derive(Debug, Clone)]
pub struct StreamedAsset {
    pub path: PathBuf,
    pub priority: StreamingPriority,
    pub handle: UntypedHandle,
}

/// Priority-ordered asset streaming system.
///
/// Call `tick()` once per frame (or on a timer) to process pending
/// requests and enforce the memory budget.
pub struct StreamingSystem {
    /// Max number of loaded assets before forced unloading occurs.
    pub max_loaded: usize,
    /// Pending load requests, ordered by priority (highest first).
    load_queue: BinaryHeap<(Reverse<StreamingPriority>, PathBuf)>,
    /// Pending unload requests.
    unload_queue: Vec<PathBuf>,
    /// Currently tracked loaded assets.
    loaded: Vec<StreamedAsset>,
    /// Number of requests processed per tick.
    pub budget_per_tick: usize,
}

impl StreamingSystem {
    pub fn new(max_loaded: usize, budget_per_tick: usize) -> Self {
        Self {
            max_loaded,
            load_queue: BinaryHeap::new(),
            unload_queue: Vec::new(),
            loaded: Vec::new(),
            budget_per_tick,
        }
    }

    /// Request that an asset at `path` be loaded with the given priority.
    pub fn request_load(&mut self, path: impl Into<PathBuf>, priority: StreamingPriority) {
        let path = path.into();
        // If already tracked at a lower priority, upgrade it.
        if let Some(a) = self.loaded.iter_mut().find(|a| a.path == path) {
            a.priority = a.priority.max(priority);
            return;
        }
        self.load_queue.push((Reverse(priority), path));
    }

    /// Request that an asset at `path` be unloaded.
    pub fn request_unload(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        self.unload_queue.push(path);
    }

    /// Remove any pending request for `path`.
    pub fn cancel(&mut self, path: impl AsRef<std::path::Path>) {
        let p = path.as_ref();
        self.load_queue.retain(|(_, q)| q != p);
        self.unload_queue.retain(|q| q != p);
    }

    /// Process the streaming queues.
    ///
    /// 1. Drains the unload queue (removes tracked entries).
    /// 2. Processes up to `budget_per_tick` load requests from the heap.
    /// 3. If `loaded.len() > max_loaded`, evicts the lowest-priority assets.
    ///
    /// Returns `(loaded_this_tick, evicted_this_tick)`.
    pub fn tick(&mut self) -> (usize, usize) {
        // Process unloads first.
        for path in self.unload_queue.drain(..) {
            self.loaded.retain(|a| a.path != path);
        }

        // Process loads up to the per-tick budget.
        let mut loaded_this_tick = 0;
        while loaded_this_tick < self.budget_per_tick {
            let Some((Reverse(priority), path)) = self.load_queue.pop() else {
                break;
            };
            // Skip if already loaded.
            if self.loaded.iter().any(|a| a.path == path) {
                continue;
            }
            self.loaded.push(StreamedAsset {
                path,
                priority,
                handle: UntypedHandle::new(0, 0), // placeholder
            });
            loaded_this_tick += 1;
        }

        // Evict if over budget.
        let mut evicted_this_tick = 0;
        while self.loaded.len() > self.max_loaded {
            let idx = self.lowest_priority_index();
            self.loaded.swap_remove(idx);
            evicted_this_tick += 1;
        }

        (loaded_this_tick, evicted_this_tick)
    }

    /// Resolve a loaded asset's placeholder handle to the real one in `server`.
    pub fn resolve_handle(&mut self, path: &PathBuf, handle: UntypedHandle) {
        if let Some(a) = self.loaded.iter_mut().find(|a| &a.path == path) {
            a.handle = handle;
        }
    }

    /// Get the `UntypedHandle` for a tracked path, if resolved.
    pub fn handle_for(&self, path: &PathBuf) -> Option<UntypedHandle> {
        self.loaded.iter().find(|a| &a.path == path).map(|a| a.handle)
    }

    /// Number of currently tracked loaded assets.
    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }

    /// Number of pending load requests.
    pub fn pending_load_count(&self) -> usize {
        self.load_queue.len()
    }

    /// Number of pending unload requests.
    pub fn pending_unload_count(&self) -> usize {
        self.unload_queue.len()
    }

    /// Iterator over all currently tracked assets.
    pub fn loaded(&self) -> impl Iterator<Item = &StreamedAsset> {
        self.loaded.iter()
    }

    /// Evict lowest-priority assets that are no longer referenced by the server.
    ///
    /// Calls `server.drain_unreferenced_all()` to clean up the underlying
    /// asset stores, then removes the corresponding streaming tracking entries.
    ///
    /// Returns the number of evicted assets.
    pub fn evict_unreferenced(&mut self, server: &mut AssetServer) -> usize {
        let evicted = server.drain_unreferenced_all();
        // Remove tracked entries whose handles became stale.
        let before = self.loaded.len();
        self.loaded.retain(|a| {
            if a.handle.index == 0 && a.handle.generation == 0 {
                return true; // still placeholder
            }
            server.get_by_path(&a.path).is_some()
        });
        before - self.loaded.len() + evicted
    }

    fn lowest_priority_index(&self) -> usize {
        self.loaded
            .iter()
            .enumerate()
            .min_by_key(|(_, a)| a.priority)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}

impl Default for StreamingSystem {
    fn default() -> Self {
        Self::new(1024, 4)
    }
}
