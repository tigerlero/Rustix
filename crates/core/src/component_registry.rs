use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ptr::NonNull;

use hecs::{Entity, World as HecsWorld};

/// Metadata and vtable for a registered component type.
///
/// Each registered component stores its size, alignment, and function pointers
/// for type-erased operations (default, clone, drop). This allows the engine
/// to reason about component types at runtime without monomorphizing on every
/// concrete type.
#[derive(Clone)]
pub struct ComponentInfo {
    pub name: &'static str,
    pub type_id: TypeId,
    pub size: usize,
    pub align: usize,
    pub(crate) make_default: fn() -> Box<dyn Any + Send + Sync>,
    pub(crate) clone_raw: unsafe fn(src: *const u8, dst: *mut u8),
    pub(crate) clone_to_box: unsafe fn(src: *const u8) -> Box<dyn Any + Send + Sync>,
    pub(crate) drop_raw: unsafe fn(ptr: *mut u8),
    /// Type-erased `hecs::World::insert_one` dispatcher.
    pub(crate) insert_fn: fn(&mut HecsWorld, Entity, Box<dyn Any + Send + Sync>),
    /// Type-erased `hecs::World::remove_one` dispatcher.
    /// Returns the removed component boxed, or None if the entity didn't have it.
    pub(crate) remove_fn: fn(&mut HecsWorld, Entity) -> Option<Box<dyn Any + Send + Sync>>,
}

impl ComponentInfo {
    /// Allocate a default component value through the type-erased vtable.
    pub fn default_value(&self) -> Box<dyn Any + Send + Sync> {
        (self.make_default)()
    }

    /// Clone the value at `src` into a new boxed allocation.
    ///
    /// # Safety
    /// `src` must point to a valid, properly-aligned instance of the registered
    /// component type.
    pub unsafe fn clone_to_boxed(&self, src: *const u8) -> Box<dyn Any + Send + Sync> {
        (self.clone_to_box)(src)
    }

    /// Insert a boxed value into `world` for `entity` using the type-erased dispatcher.
    pub fn insert_into_world(&self, world: &mut HecsWorld, entity: Entity, value: Box<dyn Any + Send + Sync>) {
        (self.insert_fn)(world, entity, value);
    }

    /// Remove this component type from `entity` in `world` via the type-erased dispatcher.
    pub fn remove_from_world(&self, world: &mut HecsWorld, entity: Entity) -> Option<Box<dyn Any + Send + Sync>> {
        (self.remove_fn)(world, entity)
    }
}

/// Registry that maps component types to their runtime metadata.
///
/// Register a component type with [`ComponentRegistry::register`] and then
/// look it up by Rust [`TypeId`] or by a string name for editor/scripting
/// integration.
#[derive(Default)]
pub struct ComponentRegistry {
    by_type_id: HashMap<TypeId, ComponentInfo>,
    by_name: HashMap<String, TypeId>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a concrete component type.
    ///
    /// `T` must be `'static` so that [`TypeId`] is well-defined, and must be
    /// `Send + Sync` because it will be stored in world-accessible buffers.
    pub fn register<T>(&mut self)
    where
        T: Any + Send + Sync + Clone + Default + 'static,
    {
        let type_id = TypeId::of::<T>();
        let name = std::any::type_name::<T>();
        // Strip the crate path so the short name is usable from the editor.
        let short_name = name.rsplit_once("::").map(|(_, s)| s).unwrap_or(name);

        let info = ComponentInfo {
            name: short_name,
            type_id,
            size: std::mem::size_of::<T>(),
            align: std::mem::align_of::<T>(),
            make_default: || Box::new(T::default()),
            clone_raw: |src, dst| unsafe {
                let val = (src as *const T).read();
                (dst as *mut T).write(val);
            },
            clone_to_box: |src| unsafe {
                let val = (src as *const T).read();
                Box::new(val) as Box<dyn Any + Send + Sync>
            },
            drop_raw: |ptr| unsafe {
                std::ptr::drop_in_place(ptr as *mut T);
            },
            insert_fn: |world, entity, val| {
                if let Ok(typed) = val.downcast::<T>() {
                    let _ = world.insert_one(entity, *typed);
                }
            },
            remove_fn: |world, entity| {
                world.remove_one::<T>(entity).ok().map(|c| Box::new(c) as Box<dyn Any + Send + Sync>)
            },
        };

        self.by_type_id.insert(type_id, info);
        self.by_name.insert(short_name.to_owned(), type_id);
    }

    /// Look up component info by its Rust [`TypeId`].
    pub fn get_by_type_id(&self, type_id: TypeId) -> Option<&ComponentInfo> {
        self.by_type_id.get(&type_id)
    }

    /// Look up component info by its registered short name.
    pub fn get_by_name(&self, name: &str) -> Option<&ComponentInfo> {
        self.by_name.get(name).and_then(|id| self.by_type_id.get(id))
    }

    /// Return the [`TypeId`] registered under `name`, if any.
    pub fn type_id_by_name(&self, name: &str) -> Option<TypeId> {
        self.by_name.get(name).copied()
    }

    /// Iterate over every registered component info.
    pub fn iter(&self) -> impl Iterator<Item = &ComponentInfo> {
        self.by_type_id.values()
    }

    /// Number of registered component types.
    pub fn len(&self) -> usize {
        self.by_type_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_type_id.is_empty()
    }

    /// Add a default component of type `name` to `entity` in `world`.
    pub fn add_component_by_name(&self, world: &mut HecsWorld, entity: Entity, name: &str) -> Result<(), String> {
        let info = self.get_by_name(name)
            .ok_or_else(|| format!("component '{}' not registered", name))?;
        let value = info.default_value();
        info.insert_into_world(world, entity, value);
        Ok(())
    }

    /// Remove component of type `name` from `entity` in `world`.
    pub fn remove_component_by_name(&self, world: &mut HecsWorld, entity: Entity, name: &str) -> Result<Option<Box<dyn Any + Send + Sync>>, String> {
        let info = self.get_by_name(name)
            .ok_or_else(|| format!("component '{}' not registered", name))?;
        Ok(info.remove_from_world(world, entity))
    }

    /// Insert all components from `bundle` into `world` for `entity`.
    ///
    /// Consumes the bundle so no intermediate clones are required.
    pub fn insert_bundle(&self, world: &mut HecsWorld, entity: Entity, bundle: DynamicBundle) -> Result<(), String> {
        for (type_id, value) in bundle.components {
            let info = self.get_by_type_id(type_id)
                .ok_or_else(|| format!("component with TypeId {:?} not registered", type_id))?;
            info.insert_into_world(world, entity, value);
        }
        Ok(())
    }
}

// --------------------------------------------------------------------------
// DynamicBundle — runtime-constructed component bundle.
// --------------------------------------------------------------------------

/// A component bundle built at runtime without compile-time type knowledge.
///
/// Each entry is a `(TypeId, boxed_value)` pair.  The [`ComponentRegistry`]
/// can apply the bundle to an entity through its type-erased dispatch table,
/// giving O(1) per-component insertion instead of a monolithic if-else chain.
#[derive(Default)]
pub struct DynamicBundle {
    components: Vec<(TypeId, Box<dyn Any + Send + Sync>)>,
}

impl DynamicBundle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a typed component to the bundle.
    pub fn add<T: Any + Send + Sync + Clone>(&mut self, component: T) {
        self.components.push((TypeId::of::<T>(), Box::new(component)));
    }

    /// Add an already-boxed component by its [`TypeId`].
    pub fn add_erased(&mut self, type_id: TypeId, value: Box<dyn Any + Send + Sync>) {
        self.components.push((type_id, value));
    }

    /// Number of components in the bundle.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// Iterate over the component entries.
    pub fn iter(&self) -> impl Iterator<Item = (TypeId, &(dyn Any + Send + Sync))> + '_ {
        self.components.iter().map(|(id, val)| (*id, val.as_ref()))
    }
}

// --------------------------------------------------------------------------
// AlignedVec — raw byte buffer with a configurable alignment.
// --------------------------------------------------------------------------

/// A growable buffer of raw bytes with a minimum alignment.
/// Used as the backing store for [`ErasedStorage`].
struct AlignedVec {
    layout_align: usize,
    ptr: NonNull<u8>,
    len: usize,
    cap: usize,
}

impl AlignedVec {
    fn new(align: usize) -> Self {
        assert!(align.is_power_of_two(), "alignment must be a power of two");
        Self {
            layout_align: align,
            ptr: NonNull::dangling(),
            len: 0,
            cap: 0,
        }
    }

    /// `element_size` is the size of one element (component).
    fn push(&mut self, src: *const u8, element_size: usize) {
        if self.len == self.cap {
            self.grow(element_size);
        }
        unsafe {
            let dst = self.ptr.as_ptr().add(self.len * element_size);
            std::ptr::copy_nonoverlapping(src, dst, element_size);
        }
        self.len += 1;
    }

    fn swap_remove(&mut self, index: usize, element_size: usize) {
        assert!(index < self.len);
        let last = self.len - 1;
        if index != last {
            unsafe {
                let dst = self.ptr.as_ptr().add(index * element_size);
                let src = self.ptr.as_ptr().add(last * element_size);
                std::ptr::copy_nonoverlapping(src, dst, element_size);
            }
        }
        self.len -= 1;
    }

    fn get(&self, index: usize, element_size: usize) -> *mut u8 {
        assert!(index < self.len);
        unsafe { self.ptr.as_ptr().add(index * element_size) }
    }

    fn grow(&mut self, element_size: usize) {
        let new_cap = if self.cap == 0 { 4 } else { self.cap * 2 };
        let new_layout =
            std::alloc::Layout::from_size_align(new_cap * element_size, self.layout_align)
                .expect("invalid layout");
        let new_ptr = if self.cap == 0 {
            unsafe { std::alloc::alloc(new_layout) }
        } else {
            let old_layout =
                std::alloc::Layout::from_size_align(self.cap * element_size, self.layout_align)
                    .expect("invalid layout");
            unsafe { std::alloc::realloc(self.ptr.as_ptr(), old_layout, new_cap * element_size) }
        };
        if new_ptr.is_null() {
            std::alloc::handle_alloc_error(new_layout);
        }
        self.ptr = NonNull::new(new_ptr).unwrap();
        self.cap = new_cap;
    }
}

impl Drop for AlignedVec {
    fn drop(&mut self) {
        if self.cap > 0 {
            // We don't know element_size here, but the caller (ErasedStorage)
            // is responsible for dropping individual elements before we reach
            // this point. We only free the raw buffer.
            unsafe {
                std::alloc::dealloc(
                    self.ptr.as_ptr(),
                    std::alloc::Layout::from_size_align(0, self.layout_align).unwrap(),
                );
            }
        }
    }
}

// --------------------------------------------------------------------------
// ErasedStorage — sparse-set storage for a single component type.
// --------------------------------------------------------------------------

/// Type-erased sparse-set storage for one component type.
///
/// Backed by a dense `AlignedVec` of raw component bytes.  Provides O(1)
/// insert, get, and remove by [`Entity`].
pub struct ErasedStorage {
    info: ComponentInfo,
    entities: Vec<Entity>,        // dense
    data: AlignedVec,            // dense bytes
    sparse: HashMap<Entity, usize>,
}

impl ErasedStorage {
    /// Create a new erased storage for the given component type.
    pub fn new(info: ComponentInfo) -> Self {
        Self {
            data: AlignedVec::new(info.align),
            info,
            entities: Vec::new(),
            sparse: HashMap::new(),
        }
    }

    /// Insert a component for `entity`.
    ///
    /// `src` must point to `info.size` bytes valid for this component type.
    /// The bytes are copied into the storage; the caller retains ownership of
    /// the source memory.
    pub fn insert(&mut self, entity: Entity, src: *const u8) {
        if let Some(&idx) = self.sparse.get(&entity) {
            // Overwrite existing.
            let ptr = self.data.get(idx, self.info.size);
            unsafe {
                (self.info.drop_raw)(ptr);
                std::ptr::copy_nonoverlapping(src, ptr, self.info.size);
            }
            return;
        }
        let idx = self.entities.len();
        self.entities.push(entity);
        self.data.push(src, self.info.size);
        self.sparse.insert(entity, idx);
    }

    /// Remove the component for `entity`, returning whether it existed.
    pub fn remove(&mut self, entity: Entity) -> bool {
        let Some(idx) = self.sparse.remove(&entity) else { return false };

        // Drop the removed element.
        let ptr = self.data.get(idx, self.info.size);
        unsafe { (self.info.drop_raw)(ptr); }

        // Swap-remove from dense arrays.
        let last = self.entities.len() - 1;
        if idx != last {
            let moved_entity = self.entities[last];
            self.entities.swap_remove(idx);
            self.data.swap_remove(idx, self.info.size);
            self.sparse.insert(moved_entity, idx);
        } else {
            self.entities.pop();
            self.data.len -= 1;
        }
        true
    }

    /// Get a raw pointer to the component data for `entity`.
    pub fn get(&self, entity: Entity) -> Option<*mut u8> {
        let &idx = self.sparse.get(&entity)?;
        Some(self.data.get(idx, self.info.size))
    }

    /// Number of entities that have this component.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Iterate over (entity, raw_ptr) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, *mut u8)> + '_ {
        self.entities.iter().enumerate().map(|(i, &e)| {
            (e, self.data.get(i, self.info.size))
        })
    }

    /// Clone the component for `entity` into a freshly-allocated boxed value.
    pub fn clone_value(&self, entity: Entity) -> Option<Box<dyn Any + Send + Sync>> {
        let ptr = self.get(entity)?;
        Some(unsafe { self.info.clone_to_boxed(ptr) })
    }
}

impl Drop for ErasedStorage {
    fn drop(&mut self) {
        // Drop every live element before freeing the raw buffer.
        for i in 0..self.entities.len() {
            let ptr = self.data.get(i, self.info.size);
            unsafe { (self.info.drop_raw)(ptr); }
        }
    }
}

// --------------------------------------------------------------------------
// ErasedWorld — helper that ties a ComponentRegistry to ErasedStorage.
// --------------------------------------------------------------------------

/// A collection of [`ErasedStorage`] buckets indexed by [`TypeId`].
///
/// This is a lightweight wrapper around one `ErasedStorage` per registered
/// component type.  It does *not* replace `hecs::World`; rather it provides
/// a side-channel for type-erased component access.
#[derive(Default)]
pub struct ErasedWorld {
    storages: HashMap<TypeId, ErasedStorage>,
}

impl ErasedWorld {
    /// Ensure a storage bucket exists for the component described by `info`.
    pub fn ensure_storage(&mut self, info: &ComponentInfo) -> &mut ErasedStorage {
        self.storages
            .entry(info.type_id)
            .or_insert_with(|| ErasedStorage::new(info.clone()))
    }

    /// Insert a component for `entity` into the appropriate storage bucket.
    ///
    /// `src` must point to `info.size` bytes.  The bytes are copied.
    pub fn insert(&mut self, info: &ComponentInfo, entity: Entity, src: *const u8) {
        self.ensure_storage(info).insert(entity, src);
    }

    /// Remove a component for `entity` from the storage bucket for `type_id`.
    pub fn remove(&mut self, type_id: TypeId, entity: Entity) -> bool {
        if let Some(storage) = self.storages.get_mut(&type_id) {
            storage.remove(entity)
        } else {
            false
        }
    }

    /// Get a raw pointer to the component data for `entity` and `type_id`.
    pub fn get(&self, type_id: TypeId, entity: Entity) -> Option<*mut u8> {
        self.storages.get(&type_id)?.get(entity)
    }

    /// Number of distinct component types with allocated storage.
    pub fn storage_count(&self) -> usize {
        self.storages.len()
    }
}

#[cfg(test)]
#[path = "component_registry_tests.rs"]
mod tests;
