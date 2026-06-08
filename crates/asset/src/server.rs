use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::Deref;
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

    /// Returns true if the asset has external references (strong_count > 1).
    fn is_referenced(&self, handle: Handle<T>) -> bool {
        if let Some(entry) = self.entries.get(handle.index() as usize) {
            if let Some(e) = entry {
                if e.generation == handle.generation() {
                    return Arc::strong_count(&e.value) > 1;
                }
            }
        }
        false
    }

    /// Remove entries that are no longer referenced by any external code.
    /// Returns the number of entries removed.
    fn drain_unreferenced(&mut self) -> usize {
        let mut removed = 0;
        for i in 0..self.entries.len() {
            if let Some(entry) = &self.entries[i] {
                if Arc::strong_count(&entry.value) == 1 {
                    self.entries[i] = None;
                    self.free_list.push(i as u32);
                    removed += 1;
                }
            }
        }
        removed
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

    /// Replace the asset at the given handle, bumping its generation.
    /// Returns the new handle if successful, or None if the handle is stale.
    fn replace(&mut self, handle: Handle<T>, asset: T) -> Option<Handle<T>> {
        let entry = self.entries.get_mut(handle.index() as usize)?.as_mut()?;
        if entry.generation != handle.generation() {
            return None;
        }
        entry.generation = entry.generation.wrapping_add(1);
        entry.value = Arc::new(asset);
        Some(Handle::new(handle.index(), entry.generation))
    }
}

/// A borrowed reference to an asset held inside the locked store.
///
/// Holds a read-lock on the `AssetStore<T>` so the entry cannot be removed
/// while the reference is alive.  Derefs to `&T` so consumers can use the
/// asset directly without cloning an `Arc`.
pub struct AssetRef<'a, T: Asset> {
    _guard: parking_lot::RwLockReadGuard<'a, AssetStore<T>>,
    ptr: *const T,
}

impl<T: Asset> Deref for AssetRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: the guard keeps the store locked, so the Arc (and therefore
        // the inner T) cannot be dropped while this reference is alive.
        unsafe { &*self.ptr }
    }
}

impl<T: Asset> std::fmt::Debug for AssetRef<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AssetRef<{}>(ptr={:?})", std::any::type_name::<T>(), self.ptr)
    }
}

/// Type-erased asset store access.
trait AnyStore: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn replace_any(&self, handle: UntypedHandle, asset: Box<dyn Any>) -> Option<UntypedHandle>;
    fn is_referenced_any(&self, handle: UntypedHandle) -> bool;
    fn drain_unreferenced_any(&self) -> usize;
}

impl<T: Asset> AnyStore for RwLock<AssetStore<T>> {
    fn as_any(&self) -> &dyn Any { self }

    fn replace_any(&self, handle: UntypedHandle, asset: Box<dyn Any>) -> Option<UntypedHandle> {
        let typed_handle = Handle::<T>::new(handle.index, handle.generation);
        let typed_asset = asset.downcast::<T>().ok()?;
        let new_handle = self.write().replace(typed_handle, *typed_asset)?;
        Some(new_handle.erase())
    }

    fn is_referenced_any(&self, handle: UntypedHandle) -> bool {
        let typed_handle = Handle::<T>::new(handle.index, handle.generation);
        self.read().is_referenced(typed_handle)
    }

    fn drain_unreferenced_any(&self) -> usize {
        self.write().drain_unreferenced()
    }
}

/// The central asset server: loads, caches, and serves assets by handle.
pub struct AssetServer {
    /// Typed asset stores, keyed by TypeId.
    stores: HashMap<TypeId, Box<dyn AnyStore>>,
    /// Maps file paths to (store TypeId, untyped handle).
    path_map: HashMap<PathBuf, (TypeId, UntypedHandle)>,
    /// Maps untyped handles to file paths.
    handle_paths: HashMap<UntypedHandle, PathBuf>,
    /// Maps an asset handle to the paths of assets it depends on.
    dependencies: HashMap<UntypedHandle, Vec<PathBuf>>,
    /// Reverse map: a dependency path -> handles of assets that need it.
    dependents: HashMap<PathBuf, Vec<UntypedHandle>>,
}

impl AssetServer {
    pub fn new() -> Self {
        Self {
            stores: HashMap::new(),
            path_map: HashMap::new(),
            handle_paths: HashMap::new(),
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
        }
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
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        let path_buf = path.as_ref().to_path_buf();
        self.path_map.insert(path_buf.clone(), (type_id, untyped));
        self.handle_paths.insert(untyped, path_buf);
        handle
    }

    /// Get an asset by handle, returning a cloned `Arc<T>`.
    ///
    /// Prefer `resolve()` for short-lived access; it avoids reference-count
    /// overhead by holding a store read-lock instead.
    pub fn get<T: Asset>(&self, handle: Handle<T>) -> Option<Arc<T>> {
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        let store = self.stores.get(&type_id)?;
        let store = store.as_any().downcast_ref::<RwLock<AssetStore<T>>>()?;
        let store = store.read();
        store.get(handle).cloned()
    }

    /// Resolve a handle to a borrowed `&T` without cloning an `Arc`.
    ///
    /// Returns an `AssetRef` that holds the store's read-lock for the lifetime
    /// of the guard.  This is the preferred handle-based access path when you
    /// only need temporary access to the asset data (8-byte handle stored in
    /// components, resolved on demand).
    pub fn resolve<T: Asset>(&self, handle: Handle<T>) -> Option<AssetRef<'_, T>> {
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        let store = self.stores.get(&type_id)?;
        let store = store.as_any().downcast_ref::<RwLock<AssetStore<T>>>()?;
        let guard = store.read();
        let entry = guard.entries.get(handle.index() as usize)?.as_ref()?;
        if entry.generation != handle.generation() {
            return None;
        }
        let ptr: *const T = Arc::as_ptr(&entry.value);
        Some(AssetRef { _guard: guard, ptr })
    }

    /// Remove an asset by handle.
    pub fn remove<T: Asset>(&mut self, handle: Handle<T>) -> bool {
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        if let Some(store) = self.stores.get(&type_id) {
            if let Some(store) = store.as_any().downcast_ref::<RwLock<AssetStore<T>>>() {
                let untyped = UntypedHandle::new(handle.index(), handle.generation());
                if let Some(path) = self.handle_paths.remove(&untyped) {
                    self.path_map.remove(&path);
                }
                return store.write().remove(handle);
            }
        }
        false
    }

    /// Replace an asset at the given handle, bumping its generation.
    /// Updates the path map with the new handle.
    pub fn replace<T: Asset>(&mut self, handle: Handle<T>, asset: T) -> Option<Handle<T>> {
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        let store = self.stores.get(&type_id)?;
        let store = store.as_any().downcast_ref::<RwLock<AssetStore<T>>>()?;
        let new_handle = store.write().replace(handle, asset)?;
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        let old_untyped = UntypedHandle::new(handle.index(), handle.generation());
        let new_untyped = UntypedHandle::new(new_handle.index(), new_handle.generation());
        if let Some(path) = self.handle_paths.remove(&old_untyped) {
            self.path_map.insert(path.clone(), (type_id, new_untyped));
            self.handle_paths.insert(new_untyped, path);
        }
        Some(new_handle)
    }

    /// Look up a handle by file path.
    pub fn get_by_path(&self, path: impl AsRef<Path>) -> Option<UntypedHandle> {
        self.path_map.get(path.as_ref()).map(|(_, h)| *h)
    }

    /// Get the path for a handle, if known.
    pub fn path_for(&self, handle: impl Into<UntypedHandle>) -> Option<&PathBuf> {
        self.handle_paths.get(&handle.into())
    }

    /// Replace an asset at an untyped handle using the correct store by TypeId.
    /// Updates path maps with the new handle. Returns the new untyped handle or None.
    pub fn replace_untyped(&mut self, handle: UntypedHandle, asset: Box<dyn Any>) -> Option<UntypedHandle> {
        let path = self.handle_paths.get(&handle)?;
        let (type_id, _) = *self.path_map.get(path)?;
        let store = self.stores.get(&type_id)?;
        let new_handle = store.replace_any(handle, asset)?;
        if let Some(path) = self.handle_paths.remove(&handle) {
            self.path_map.insert(path.clone(), (type_id, new_handle));
            self.handle_paths.insert(new_handle, path);
        }
        Some(new_handle)
    }

    /// Check whether an asset still has external references (strong_count > 1).
    pub fn is_referenced<T: Asset>(&self, handle: Handle<T>) -> bool {
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        if let Some(store) = self.stores.get(&type_id) {
            if let Some(store) = store.as_any().downcast_ref::<RwLock<AssetStore<T>>>() {
                return store.read().is_referenced(handle);
            }
        }
        false
    }

    /// Remove all assets of type `T` that are no longer referenced externally.
    /// Returns the number of entries removed.
    pub fn drain_unreferenced<T: Asset>(&mut self) -> usize {
        let type_id = TypeId::of::<RwLock<AssetStore<T>>>();
        let removed = if let Some(store) = self.stores.get(&type_id) {
            store.drain_unreferenced_any()
        } else {
            0
        };
        removed
    }

    /// Remove all assets across all stores that are no longer referenced.
    /// Returns the total number of entries removed.
    pub fn drain_unreferenced_all(&mut self) -> usize {
        let mut total = 0;
        for store in self.stores.values() {
            total += store.drain_unreferenced_any();
        }
        total
    }

    /// Returns the total number of loaded assets across all types.
    pub fn asset_count(&self) -> usize {
        self.stores.len()
    }

    // ── Dependency tracking ──

    /// Declare that `handle` depends on the assets at the given file paths.
    ///
    /// Call this after inserting an asset so the server knows which other
    /// assets must be present before this one is considered fully resolved.
    /// The reverse `dependents` map is also updated so that when a dependency
    /// is loaded later, all waiting assets can be notified.
    pub fn declare_dependencies<T: Asset>(&mut self, handle: Handle<T>, deps: &[impl AsRef<Path>]) {
        let untyped = handle.erase();
        let paths: Vec<PathBuf> = deps.iter().map(|p| p.as_ref().to_path_buf()).collect();
        for path in &paths {
            self.dependents.entry(path.clone()).or_default().push(untyped);
        }
        self.dependencies.insert(untyped, paths);
    }

    /// Get the dependency paths registered for an asset.
    pub fn dependency_paths<T: Asset>(&self, handle: Handle<T>) -> Option<&[PathBuf]> {
        self.dependencies.get(&handle.erase()).map(|v| v.as_slice())
    }

    /// Check whether every dependency of `handle` has already been loaded
    /// into the server (i.e. each dependency path exists in `path_map`).
    pub fn are_dependencies_loaded<T: Asset>(&self, handle: Handle<T>) -> bool {
        let Some(paths) = self.dependencies.get(&handle.erase()) else {
            return true;
        };
        paths.iter().all(|p| self.path_map.contains_key(p))
    }

    /// Resolve dependency paths to their loaded handles.
    ///
    /// Returns `None` if any dependency is not yet loaded.
    pub fn resolve_dependencies<T: Asset>(&self, handle: Handle<T>) -> Option<Vec<UntypedHandle>> {
        let paths = self.dependencies.get(&handle.erase())?;
        paths.iter().map(|p| self.path_map.get(p).map(|(_, h)| *h)).collect()
    }

    /// Get all asset handles that declared a dependency on `path`.
    pub fn dependents_of(&self, path: impl AsRef<Path>) -> Option<&[UntypedHandle]> {
        self.dependents.get(path.as_ref()).map(|v| v.as_slice())
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

    pub fn path_map(&self) -> &HashMap<PathBuf, (TypeId, UntypedHandle)> {
        &self.path_map
    }
}

impl Default for AssetServer {
    fn default() -> Self { Self::new() }
}
