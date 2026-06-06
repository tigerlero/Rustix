use std::alloc::{self, Layout};
use std::collections::HashMap;
use std::marker::PhantomData;

use hecs::Entity;

// ------------------------------------------------------------------
// AlignedVec: a growable buffer with a fixed element size and alignment.
// ------------------------------------------------------------------

/// Raw growable buffer with user-specified element size and alignment.
///
/// Stores `len` elements, each of `element_size` bytes, aligned to `align`.
/// The backing memory is allocated with the system allocator so the first
/// element is always properly aligned for SIMD or GPU upload.
pub struct AlignedVec {
    ptr: *mut u8,
    len: usize,
    capacity: usize,
    element_size: usize,
    align: usize,
}

impl AlignedVec {
    pub fn new(element_size: usize, align: usize) -> Self {
        assert!(element_size > 0);
        assert!(align.is_power_of_two());
        Self {
            ptr: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
            element_size,
            align,
        }
    }

    fn layout(capacity: usize, element_size: usize, align: usize) -> Layout {
        Layout::from_size_align(capacity * element_size, align).unwrap()
    }

    fn ensure_capacity(&mut self, needed: usize) {
        if needed <= self.capacity {
            return;
        }
        let new_cap = needed.max(self.capacity * 2).max(8);
        let new_layout = Self::layout(new_cap, self.element_size, self.align);
        if self.capacity == 0 {
            self.ptr = unsafe { alloc::alloc(new_layout) };
        } else {
            let old_layout = Self::layout(self.capacity, self.element_size, self.align);
            self.ptr = unsafe { alloc::realloc(self.ptr, old_layout, new_layout.size()) };
        }
        if self.ptr.is_null() {
            alloc::handle_alloc_error(new_layout);
        }
        self.capacity = new_cap;
    }

    /// Append `element_size` bytes copied from `src`.
    pub fn push_raw(&mut self, src: *const u8) {
        self.ensure_capacity(self.len + 1);
        let dst = unsafe { self.ptr.add(self.len * self.element_size) };
        unsafe {
            std::ptr::copy_nonoverlapping(src, dst, self.element_size);
        }
        self.len += 1;
    }

    /// Overwrite the element at `index` with `element_size` bytes from `src`.
    pub fn write_raw(&mut self, index: usize, src: *const u8) {
        assert!(index < self.len);
        let dst = unsafe { self.ptr.add(index * self.element_size) };
        unsafe {
            std::ptr::copy_nonoverlapping(src, dst, self.element_size);
        }
    }

    /// Copy the element at `index` into `dst`.
    pub fn read_raw(&self, index: usize, dst: *mut u8) {
        assert!(index < self.len);
        let src = unsafe { self.ptr.add(index * self.element_size) };
        unsafe {
            std::ptr::copy_nonoverlapping(src, dst, self.element_size);
        }
    }

    /// Pointer to element `index`.
    pub fn element_ptr(&self, index: usize) -> *mut u8 {
        assert!(index < self.len);
        unsafe { self.ptr.add(index * self.element_size) }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Decrement the logical length by one (used for swap-remove).
    pub fn pop(&mut self) {
        assert!(!self.is_empty());
        self.len -= 1;
    }

    /// Return a typed slice covering all elements.
    ///
    /// # Safety
    /// `T` must match the element size and alignment stored in this buffer.
    /// Violating this is undefined behaviour.
    pub unsafe fn as_slice<T>(&self) -> &[T] {
        assert_eq!(std::mem::size_of::<T>(), self.element_size);
        std::slice::from_raw_parts(self.ptr as *const T, self.len)
    }

    /// Return a mutable typed slice covering all elements.
    ///
    /// # Safety
    /// `T` must match the element size and alignment stored in this buffer.
    pub unsafe fn as_slice_mut<T>(&mut self) -> &mut [T] {
        assert_eq!(std::mem::size_of::<T>(), self.element_size);
        std::slice::from_raw_parts_mut(self.ptr as *mut T, self.len)
    }
}

impl Drop for AlignedVec {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.capacity > 0 {
            let layout = Self::layout(self.capacity, self.element_size, self.align);
            unsafe {
                alloc::dealloc(self.ptr, layout);
            }
        }
    }
}

// ------------------------------------------------------------------
// SoA layout and storage
// ------------------------------------------------------------------

/// Description of one field in a SoA layout.
#[derive(Debug, Clone)]
pub struct SoAField {
    pub name: &'static str,
    pub size: usize,
    pub align: usize,
}

/// Structure-of-Arrays storage for a single component type.
///
/// Each field of the component is stored in its own contiguous,
/// aligned buffer so iterating over a single field touches only
/// that field's cache lines.
///
/// # Example
///
/// ```rust
/// use rustix_core::soa_storage::{SoAField, SoAStorage};
///
/// // Define a SoA layout equivalent to:
/// // struct Particle { position: [f32; 3], velocity: [f32; 3], life: f32 }
/// let layout = vec![
///     SoAField { name: "position", size: 12, align: 4 },
///     SoAField { name: "velocity", size: 12, align: 4 },
///     SoAField { name: "life",     size: 4,  align: 4 },
/// ];
/// let mut storage = SoAStorage::new(layout);
/// ```
pub struct SoAStorage {
    fields: Vec<SoAField>,
    buffers: Vec<AlignedVec>,
    entity_to_slot: HashMap<Entity, usize>,
    slot_to_entity: Vec<Entity>,
}

impl SoAStorage {
    /// Create a new SoA storage with the given field layout.
    pub fn new(fields: Vec<SoAField>) -> Self {
        let buffers: Vec<AlignedVec> = fields
            .iter()
            .map(|f| AlignedVec::new(f.size, f.align))
            .collect();
        Self {
            fields,
            buffers,
            entity_to_slot: HashMap::new(),
            slot_to_entity: Vec::new(),
        }
    }

    /// Number of entities stored.
    pub fn len(&self) -> usize {
        self.slot_to_entity.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of fields in the layout.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Insert or update an entity.  `component_bytes` must be the
    /// full, packed representation of the component in the same order
    /// as the fields passed to `new`.
    pub fn insert(&mut self, entity: Entity, component_bytes: &[u8]) {
        let total_size: usize = self.fields.iter().map(|f| f.size).sum();
        assert_eq!(
            component_bytes.len(),
            total_size,
            "component_bytes length must equal sum of field sizes"
        );

        if let Some(&slot) = self.entity_to_slot.get(&entity) {
            // Update existing
            let mut offset = 0usize;
            for (i, field) in self.fields.iter().enumerate() {
                self.buffers[i].write_raw(slot, &component_bytes[offset]);
                offset += field.size;
            }
        } else {
            // Append new
            let slot = self.slot_to_entity.len();
            self.entity_to_slot.insert(entity, slot);
            self.slot_to_entity.push(entity);
            let mut offset = 0usize;
            for (i, field) in self.fields.iter().enumerate() {
                self.buffers[i].push_raw(&component_bytes[offset]);
                offset += field.size;
            }
        }
    }

    /// Remove an entity.  Returns `true` if it existed.
    ///
    /// Uses swap-remove to keep buffers dense.
    pub fn remove(&mut self, entity: Entity) -> bool {
        let slot = match self.entity_to_slot.remove(&entity) {
            Some(s) => s,
            None => return false,
        };

        let last_slot = self.slot_to_entity.len() - 1;
        if slot != last_slot {
            // Swap with last element to keep buffers dense
            for buf in &mut self.buffers {
                let src = buf.element_ptr(last_slot);
                buf.write_raw(slot, src);
            }
            let moved_entity = self.slot_to_entity[last_slot];
            self.slot_to_entity[slot] = moved_entity;
            self.entity_to_slot.insert(moved_entity, slot);
        }
        self.slot_to_entity.pop();
        for buf in &mut self.buffers {
            buf.pop();
        }
        true
    }

    /// Get the slot index for an entity.
    pub fn slot(&self, entity: Entity) -> Option<usize> {
        self.entity_to_slot.get(&entity).copied()
    }

    /// Read one field of one entity into `dst`.
    pub fn read_field(&self, entity: Entity, field_index: usize, dst: &mut [u8]) {
        let slot = self.slot(entity).expect("entity not in storage");
        let field = &self.fields[field_index];
        assert_eq!(dst.len(), field.size);
        self.buffers[field_index].read_raw(slot, dst.as_mut_ptr());
    }

    /// Write one field of one entity from `src`.
    pub fn write_field(&mut self, entity: Entity, field_index: usize, src: &[u8]) {
        let slot = self.slot(entity).expect("entity not in storage");
        let field = &self.fields[field_index];
        assert_eq!(src.len(), field.size);
        self.buffers[field_index].write_raw(slot, src.as_ptr());
    }

    /// Typed access to a field buffer.
    ///
    /// # Safety
    /// `T` must match the field's size and alignment.
    pub unsafe fn field_slice<T>(&self, field_index: usize) -> &[T] {
        self.buffers[field_index].as_slice::<T>()
    }

    /// Mutable typed access to a field buffer.
    ///
    /// # Safety
    /// `T` must match the field's size and alignment.
    pub unsafe fn field_slice_mut<T>(&mut self, field_index: usize) -> &mut [T] {
        self.buffers[field_index].as_slice_mut::<T>()
    }

    /// Iterator over (entity, slot) pairs.
    pub fn entities(&self) -> impl Iterator<Item = (Entity, usize)> + '_ {
        self.slot_to_entity
            .iter()
            .enumerate()
            .map(|(slot, &entity)| (entity, slot))
    }
}

/// Registry that owns multiple `SoAStorage` instances keyed by a user-defined name.
#[derive(Default)]
pub struct SoARegistry {
    storages: HashMap<String, SoAStorage>,
}

impl SoARegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: impl Into<String>, storage: SoAStorage) {
        self.storages.insert(name.into(), storage);
    }

    pub fn get(&self, name: &str) -> Option<&SoAStorage> {
        self.storages.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut SoAStorage> {
        self.storages.get_mut(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<SoAStorage> {
        self.storages.remove(name)
    }

    pub fn len(&self) -> usize {
        self.storages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.storages.is_empty()
    }
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
#[path = "soa_storage_tests.rs"]
mod tests;
