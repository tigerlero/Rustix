use hecs::Entity;
use std::any::TypeId;
use std::collections::{HashMap, HashSet};

/// Tracks which entities had their components modified during the current tick.
///
/// Because `hecs` does not expose mutation hooks, changes must be flagged
/// explicitly via [`ChangeTracker::flag`] or [`ChangeTracker::flag_erased`].
/// At the end of a tick the engine calls [`ChangeTracker::clear`] so that
/// the next tick starts with a clean slate.
#[derive(Default)]
pub struct ChangeTracker {
    /// Maps `TypeId` → set of entities whose component of that type changed.
    changed: HashMap<TypeId, HashSet<Entity>>,
}

impl ChangeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark `entity` as having a changed component of type `T`.
    pub fn flag<T: 'static>(&mut self, entity: Entity) {
        self.flag_erased(TypeId::of::<T>(), entity);
    }

    /// Mark `entity` as having a changed component described by `type_id`.
    pub fn flag_erased(&mut self, type_id: TypeId, entity: Entity) {
        self.changed
            .entry(type_id)
            .or_default()
            .insert(entity);
    }

    /// Check whether `entity` has a changed component of type `T`.
    pub fn is_changed<T: 'static>(&self, entity: Entity) -> bool {
        self.is_changed_erased(TypeId::of::<T>(), entity)
    }

    /// Check whether `entity` has a changed component with `type_id`.
    pub fn is_changed_erased(&self, type_id: TypeId, entity: Entity) -> bool {
        self.changed
            .get(&type_id)
            .map(|set| set.contains(&entity))
            .unwrap_or(false)
    }

    /// Return the set of entities with a changed component of type `T`, if any.
    pub fn changed_entities<T: 'static>(&self) -> Option<&HashSet<Entity>> {
        self.changed_entities_erased(TypeId::of::<T>())
    }

    /// Return the set of entities with a changed component of `type_id`, if any.
    pub fn changed_entities_erased(&self, type_id: TypeId) -> Option<&HashSet<Entity>> {
        self.changed.get(&type_id)
    }

    /// Number of distinct component types that have tracked changes.
    pub fn tracked_type_count(&self) -> usize {
        self.changed.len()
    }

    /// Total number of changed entity/component pairs across all types.
    pub fn total_changed_count(&self) -> usize {
        self.changed.values().map(|set| set.len()).sum()
    }

    /// Remove all tracked changes.  Typically called once per tick.
    pub fn clear(&mut self) {
        self.changed.clear();
    }

    /// Remove tracked changes for a single component type.
    pub fn clear_type<T: 'static>(&mut self) {
        self.changed.remove(&TypeId::of::<T>());
    }

    /// Remove tracked changes for a single component type by its [`TypeId`].
    pub fn clear_type_erased(&mut self, type_id: TypeId) {
        self.changed.remove(&type_id);
    }
}
