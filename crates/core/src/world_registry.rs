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
