use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;

use crate::handle::{Asset, Handle, UntypedHandle};

/// Represents a stored asset entry with its generation counter.
struct AssetEntry<T: Asset> {
    value: Arc<T>,
    generation: u32,
}

/// Stores all assets of a single type.
struct AssetStore<T: Asset> {
    entries: Vec<Option<AssetEntry<T>>>,
    free_list: Vec<u32>,
    generation: u32,
}

impl<T: Asset> AssetStore<T> {
    fn new() -> Self {
        Self { entries: Vec::new(), free_list: Vec::new(), generation: 0 }
    }

    fn insert(&mut self, asset: T) -> Handle<T> {
        self.generation = self.generation.wrapping_add(1);

        if let Some(&idx) = self.free_list.last() {
            self.free_list.pop();
            let index = idx as usize;
            let generation = self.entries[index].as_ref().map_or(0, |e| e.generation.wrapping_add(1));
            let entry = AssetEntry { value: Arc::new(asset), generation };
            self.entries[index] = Some(entry);
            Handle::new(index as u32, generation)
        } else {
            let index = self.entries.len() as u32;
            let entry = AssetEntry { value: Arc::new(asset), generation: 0 };
            self.entries.push(Some(entry));
            Handle::new(index, 0)
        }
    }

    fn get(&self, handle: Handle<T>) -> Option<&Arc<T>> {
        let entry = self.entries.get(handle.index() as usize)?.as_ref()?;
        if entry.generation == handle.generation() {
            Some(&entry.value)
        } else {
            None
        }
    }

    fn remove(&mut self, handle: Handle<T>) -> bool {
        if let Some(entry) = self.entries.get_mut(handle.index() as usize) {
            if let Some(e) = entry {
                if e.generation == handle.generation() {
                    *entry = None;
                    self.free_list.push(handle.index());
                    return true;
                }
            }
        }
        false
    }
}

/// TType-erased asset store access.
trait AnyStore: Send + Sync {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Asset> AnyStore for RwLock<AssetStore<T>> {
    fn as_any(&self) -> &dyn Any { self }
}

/// The central asset server: loads, caches, and serves assets by handle.
pub struct AssetServer {
    /// Typed asset stores, keyed by TypeId.
    stores: HashMap<TypeId, Box<dyn AnyStore>>,
    /// Maps file paths to untyped handles.
    path_map: HashMap<PathBuf, UntypedHandle>,
    /// Maps untyped handles to file paths.
    handle_paths: HashMap<UntypedHandle, PathBuf>,
}

impl AssetServer {
    pub fn new() -> Self {
        Self { stores: HashMap::new(), path_map: HashMap::new(), handle_paths: HashMap::new() }
    }

    /// Insert an asset directly, returning a typed handle.
    pub fn insert<T: Asset>(&mut self, asset: T) -> Handle<T> {
        let store = self.get_or_create_store::<T>();
        let mut store = store.write();
        store.insert(asset)
    }

    /// Insert an asset with a path mapping.
    pub fn insert_with_path<T: Asset>(&mut self, path: impl AsRef<Path>, asset: T) -> Handle<T> {
        let handle = self.insert(asset);
        let untyped = UntypedHandle::new(handle.index(), handle.generation());
        let path_buf = path.as_ref().to_path_buf();
        self.path_map.insert(path_buf.clone(), untyped);
        self.handle_paths.insert(untyped, path_buf);
        handle
    }

    /// Get an asset by handle. Returns None if handle is stale.
    pub fn get<T: Asset>(&self, handle: Handle<T>) -> Option<Arc<T>> {
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        let store = self.stores.get(&type_id)?;
        let store = store.as_any().downcast_ref::<RwLock<AssetStore<T>>>()?;
        let store = store.read();
        store.get(handle).cloned()
    }

    /// Remove an asset by handle.
    pub fn remove<T: Asset>(&mut self, handle: Handle<T>) -> bool {
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        if let Some(store) = self.stores.get(&type_id) {
            if let Some(store) = store.as_any().downcast_ref::<RwLock<AssetStore<T>>>() {
                let untyped = UntypedHandle::new(handle.index(), handle.generation());
                self.handle_paths.remove(&untyped);
                return store.write().remove(handle);
            }
        }
        false
    }

    /// Look up a handle by file path.
    pub fn get_by_path(&self, path: impl AsRef<Path>) -> Option<UntypedHandle> {
        self.path_map.get(path.as_ref()).copied()
    }

    /// Get the path for a handle, if known.
    pub fn path_for(&self, handle: impl Into<UntypedHandle>) -> Option<&PathBuf> {
        self.handle_paths.get(&handle.into())
    }

    /// Returns the total number of loaded assets across all types.
    pub fn asset_count(&self) -> usize {
        self.stores.len()
    }

    fn get_or_create_store<T: Asset>(&mut self) -> &RwLock<AssetStore<T>> {
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        if !self.stores.contains_key(&type_id) {
            let store: RwLock<AssetStore<T>> = RwLock::new(AssetStore::new());
            self.stores.insert(type_id, Box::new(store));
        }
        self.stores[&type_id]
            .as_any()
            .downcast_ref::<RwLock<AssetStore<T>>>()
            .unwrap()
    }

    pub fn path_map(&self) -> &HashMap<PathBuf, UntypedHandle> {
        &self.path_map
    }
}

impl Default for AssetServer {
    fn default() -> Self { Self::new() }
}
