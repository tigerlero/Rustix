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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::EcsWorld;

    #[derive(Debug, Clone, PartialEq, Default)]
    struct Position {
        x: f32,
        y: f32,
        z: f32,
    }

    #[derive(Debug, Clone, PartialEq, Default)]
    struct Velocity {
        dx: f32,
        dy: f32,
    }

    #[test]
    fn tracker_starts_empty() {
        let tracker = ChangeTracker::new();
        assert_eq!(tracker.tracked_type_count(), 0);
        assert_eq!(tracker.total_changed_count(), 0);
    }

    #[test]
    fn tracker_flag_and_check() {
        let mut tracker = ChangeTracker::new();
        let mut world = EcsWorld::new();
        let e = world.spawn(());

        tracker.flag::<Position>(e);
        assert!(tracker.is_changed::<Position>(e));
        assert!(!tracker.is_changed::<Velocity>(e));
    }

    #[test]
    fn tracker_flag_erased() {
        let mut tracker = ChangeTracker::new();
        let mut world = EcsWorld::new();
        let e = world.spawn(());

        tracker.flag_erased(TypeId::of::<Position>(), e);
        assert!(tracker.is_changed_erased(TypeId::of::<Position>(), e));
        assert!(!tracker.is_changed_erased(TypeId::of::<Velocity>(), e));
    }

    #[test]
    fn tracker_changed_entities() {
        let mut tracker = ChangeTracker::new();
        let mut world = EcsWorld::new();
        let e1 = world.spawn(());
        let e2 = world.spawn(());

        tracker.flag::<Position>(e1);
        tracker.flag::<Position>(e2);

        let set = tracker.changed_entities::<Position>().unwrap();
        assert!(set.contains(&e1));
        assert!(set.contains(&e2));
    }

    #[test]
    fn tracker_manual_filter_with_is_changed() {
        let mut tracker = ChangeTracker::new();
        let mut world = EcsWorld::new();
        let e1 = world.spawn((Position { x: 1.0, y: 0.0, z: 0.0 },));
        let e2 = world.spawn((Position { x: 2.0, y: 0.0, z: 0.0 },));
        let e3 = world.spawn((Position { x: 3.0, y: 0.0, z: 0.0 },));

        tracker.flag::<Position>(e1);
        tracker.flag::<Position>(e3);

        let mut changed = Vec::new();
        for (e, _pos) in world.query_mut::<(Entity, &Position)>() {
            if tracker.is_changed::<Position>(e) {
                changed.push(e);
            }
        }
        assert_eq!(changed.len(), 2);
        assert!(changed.contains(&e1));
        assert!(!changed.contains(&e2));
        assert!(changed.contains(&e3));
    }

    #[test]
    fn tracker_clear_removes_all() {
        let mut tracker = ChangeTracker::new();
        let mut world = EcsWorld::new();
        let e = world.spawn(());

        tracker.flag::<Position>(e);
        tracker.flag::<Velocity>(e);
        assert_eq!(tracker.total_changed_count(), 2);

        tracker.clear();
        assert!(!tracker.is_changed::<Position>(e));
        assert!(!tracker.is_changed::<Velocity>(e));
        assert_eq!(tracker.total_changed_count(), 0);
    }

    #[test]
    fn tracker_clear_type_selective() {
        let mut tracker = ChangeTracker::new();
        let mut world = EcsWorld::new();
        let e = world.spawn(());

        tracker.flag::<Position>(e);
        tracker.flag::<Velocity>(e);

        tracker.clear_type::<Position>();
        assert!(!tracker.is_changed::<Position>(e));
        assert!(tracker.is_changed::<Velocity>(e));
    }

    #[test]
    fn tracker_clear_type_erased() {
        let mut tracker = ChangeTracker::new();
        let mut world = EcsWorld::new();
        let e = world.spawn(());

        tracker.flag::<Position>(e);
        tracker.flag::<Velocity>(e);

        tracker.clear_type_erased(TypeId::of::<Position>());
        assert!(!tracker.is_changed::<Position>(e));
        assert!(tracker.is_changed::<Velocity>(e));
    }

    #[test]
    fn tracker_multiple_entities_same_type() {
        let mut tracker = ChangeTracker::new();
        let mut world = EcsWorld::new();
        let e1 = world.spawn(());
        let e2 = world.spawn(());
        let e3 = world.spawn(());

        tracker.flag::<Position>(e1);
        tracker.flag::<Position>(e3);

        assert_eq!(tracker.changed_entities::<Position>().unwrap().len(), 2);
        assert!(tracker.is_changed::<Position>(e1));
        assert!(!tracker.is_changed::<Position>(e2));
        assert!(tracker.is_changed::<Position>(e3));
    }

    #[test]
    fn tracker_duplicate_flag_is_idempotent() {
        let mut tracker = ChangeTracker::new();
        let mut world = EcsWorld::new();
        let e = world.spawn(());

        tracker.flag::<Position>(e);
        tracker.flag::<Position>(e);
        tracker.flag::<Position>(e);

        assert_eq!(tracker.changed_entities::<Position>().unwrap().len(), 1);
    }

    #[test]
    fn tracker_changed_entities_unknown_type_returns_none() {
        let tracker = ChangeTracker::new();
        assert!(tracker.changed_entities::<Position>().is_none());
    }
}
