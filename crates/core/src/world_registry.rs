use std::collections::HashMap;

use hecs::{Entity, World as HecsWorld};

/// Named collection of independent ECS worlds (game, editor, preview, etc.).
///
/// Only one world is "active" at a time for the current context, but all
/// worlds remain alive in the registry so they can be switched between
/// instantly.
#[derive(Default)]
pub struct WorldRegistry {
    worlds: HashMap<String, HecsWorld>,
    active: Option<String>,
}

impl WorldRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new empty world with `name` and make it active.
    pub fn create(&mut self, name: impl Into<String>) -> &mut HecsWorld {
        let name = name.into();
        self.worlds.insert(name.clone(), HecsWorld::new());
        self.active = Some(name.clone());
        self.worlds.get_mut(&name).unwrap()
    }

    /// Create a new empty world without changing the active one.
    pub fn create_inactive(&mut self, name: impl Into<String>) {
        self.worlds.insert(name.into(), HecsWorld::new());
    }

    /// Destroy a world by name.  Returns `true` if it existed.
    pub fn destroy(&mut self, name: &str) -> bool {
        let removed = self.worlds.remove(name).is_some();
        if self.active.as_deref() == Some(name) {
            self.active = None;
        }
        removed
    }

    /// Get a reference to the world named `name`.
    pub fn get(&self, name: &str) -> Option<&HecsWorld> {
        self.worlds.get(name)
    }

    /// Get a mutable reference to the world named `name`.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut HecsWorld> {
        self.worlds.get_mut(name)
    }

    /// Set the active world by name.  Panics if the world does not exist.
    pub fn set_active(&mut self, name: &str) {
        assert!(
            self.worlds.contains_key(name),
            "world '{}' does not exist",
            name
        );
        self.active = Some(name.to_string());
    }

    /// Reference to the currently active world, if any.
    pub fn active(&self) -> Option<&HecsWorld> {
        self.active.as_ref().and_then(|n| self.worlds.get(n))
    }

    /// Mutable reference to the currently active world, if any.
    pub fn active_mut(&mut self) -> Option<&mut HecsWorld> {
        let name = self.active.as_ref()?;
        self.worlds.get_mut(name)
    }

    /// Convenience: spawn into the active world.
    pub fn spawn_active<T: hecs::DynamicBundle>(
        &mut self,
        components: T,
    ) -> Option<Entity> {
        Some(self.active_mut()?.spawn(components))
    }

    /// List all world names.
    pub fn names(&self) -> impl Iterator<Item = &str> + '_ {
        self.worlds.keys().map(|s| s.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.worlds.is_empty()
    }

    pub fn len(&self) -> usize {
        self.worlds.len()
    }

    pub fn has(&self, name: &str) -> bool {
        self.worlds.contains_key(name)
    }

    /// Clear all worlds and reset active state.
    pub fn clear(&mut self) {
        self.worlds.clear();
        self.active = None;
    }
}

/// Bidirectional mapping between entities in two different worlds.
///
/// Useful for editor/preview scenarios where the editor needs to know
/// which entity in the preview world corresponds to an entity in the
/// game world.
#[derive(Default)]
pub struct EntityMapping {
    /// game_entity → preview_entity
    forward: HashMap<Entity, Entity>,
    /// preview_entity → game_entity
    reverse: HashMap<Entity, Entity>,
}

impl EntityMapping {
    pub fn new() -> Self {
        Self::default()
    }

    /// Map `src` in the source world to `dst` in the target world.
    pub fn insert(&mut self, src: Entity, dst: Entity) {
        // remove any existing mapping for src
        if let Some(old) = self.forward.remove(&src) {
            self.reverse.remove(&old);
        }
        // remove any existing mapping for dst
        if let Some(old) = self.reverse.remove(&dst) {
            self.forward.remove(&old);
        }
        self.forward.insert(src, dst);
        self.reverse.insert(dst, src);
    }

    /// Get the target entity for `src`.
    pub fn get(&self, src: Entity) -> Option<Entity> {
        self.forward.get(&src).copied()
    }

    /// Get the source entity for `dst`.
    pub fn get_reverse(&self, dst: Entity) -> Option<Entity> {
        self.reverse.get(&dst).copied()
    }

    /// Remove the mapping for `src` (and its reverse).
    pub fn remove(&mut self, src: Entity) -> Option<Entity> {
        let dst = self.forward.remove(&src)?;
        self.reverse.remove(&dst);
        Some(dst)
    }

    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }

    pub fn len(&self) -> usize {
        self.forward.len()
    }

    pub fn clear(&mut self) {
        self.forward.clear();
        self.reverse.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_starts_empty() {
        let reg = WorldRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.active().is_none());
    }

    #[test]
    fn registry_create_makes_active() {
        let mut reg = WorldRegistry::new();
        let world = reg.create("game");
        let e = world.spawn((42i32,));
        assert_eq!(reg.active().unwrap().query::<&i32>().iter().count(), 1);
        assert!(reg.has("game"));
        assert!(!reg.has("editor"));
    }

    #[test]
    fn registry_create_inactive() {
        let mut reg = WorldRegistry::new();
        reg.create("game");
        reg.create_inactive("editor");
        assert_eq!(reg.len(), 2);
        assert_eq!(reg.active().unwrap().query::<&i32>().iter().count(), 0);
    }

    #[test]
    fn registry_set_active() {
        let mut reg = WorldRegistry::new();
        reg.create("game");
        reg.create_inactive("editor");
        reg.get_mut("editor").unwrap().spawn((42i32,));
        reg.set_active("editor");
        assert_eq!(reg.active().unwrap().query::<&i32>().iter().count(), 1);
    }

    #[test]
    #[should_panic(expected = "world 'ghost' does not exist")]
    fn registry_set_active_unknown_panics() {
        let mut reg = WorldRegistry::new();
        reg.set_active("ghost");
    }

    #[test]
    fn registry_destroy_removes_world() {
        let mut reg = WorldRegistry::new();
        reg.create("game");
        assert!(reg.destroy("game"));
        assert!(!reg.has("game"));
        assert!(reg.active().is_none());
    }

    #[test]
    fn registry_destroy_unknown_returns_false() {
        let mut reg = WorldRegistry::new();
        assert!(!reg.destroy("ghost"));
    }

    #[test]
    fn registry_get_and_get_mut() {
        let mut reg = WorldRegistry::new();
        reg.create("game");
        assert!(reg.get("game").is_some());
        reg.get_mut("game").unwrap().spawn((42i32,));
        assert_eq!(reg.get("game").unwrap().query::<&i32>().iter().count(), 1);
    }

    #[test]
    fn registry_spawn_active() {
        let mut reg = WorldRegistry::new();
        reg.create("game");
        let e = reg.spawn_active((42i32,)).unwrap();
        assert!(reg.active().unwrap().satisfies::<&i32>(e));
    }

    #[test]
    fn registry_spawn_active_none_when_no_active() {
        let mut reg = WorldRegistry::new();
        reg.create_inactive("game");
        assert!(reg.spawn_active((42i32,)).is_none());
    }

    #[test]
    fn registry_names() {
        let mut reg = WorldRegistry::new();
        reg.create("game");
        reg.create_inactive("editor");
        let mut names: Vec<_> = reg.names().collect();
        names.sort();
        assert_eq!(names, vec!["editor", "game"]);
    }

    #[test]
    fn registry_clear() {
        let mut reg = WorldRegistry::new();
        reg.create("game");
        reg.create_inactive("editor");
        reg.clear();
        assert!(reg.is_empty());
        assert!(reg.active().is_none());
    }

    // ------------------------------------------------------------------
    // EntityMapping tests
    // ------------------------------------------------------------------

    #[test]
    fn mapping_insert_and_get() {
        let mut map = EntityMapping::new();
        let mut w = HecsWorld::new();
        let e1 = w.spawn(());
        let e2 = w.spawn(());
        map.insert(e1, e2);
        assert_eq!(map.get(e1), Some(e2));
        assert_eq!(map.get_reverse(e2), Some(e1));
    }

    #[test]
    fn mapping_overwrite_existing() {
        let mut map = EntityMapping::new();
        let mut w = HecsWorld::new();
        let e1 = w.spawn(());
        let e2 = w.spawn(());
        let e3 = w.spawn(());
        map.insert(e1, e2);
        map.insert(e1, e3);
        assert_eq!(map.get(e1), Some(e3));
        assert!(map.get(e2).is_none());
        assert_eq!(map.get_reverse(e3), Some(e1));
    }

    #[test]
    fn mapping_remove() {
        let mut map = EntityMapping::new();
        let mut w = HecsWorld::new();
        let e1 = w.spawn(());
        let e2 = w.spawn(());
        map.insert(e1, e2);
        assert_eq!(map.remove(e1), Some(e2));
        assert!(map.get(e1).is_none());
        assert!(map.get_reverse(e2).is_none());
    }

    #[test]
    fn mapping_len_and_clear() {
        let mut map = EntityMapping::new();
        let mut w = HecsWorld::new();
        let e1 = w.spawn(());
        let e2 = w.spawn(());
        map.insert(e1, e2);
        assert_eq!(map.len(), 1);
        map.clear();
        assert!(map.is_empty());
    }
}
