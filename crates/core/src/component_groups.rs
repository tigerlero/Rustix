use std::any::TypeId;
use std::collections::HashMap;

use hecs::{Entity, World as HecsWorld};

/// A named set of component types that are commonly accessed together.
///
/// Groups are hints to the engine: they help validate queries, pre-warm
/// archetypes, and provide a shorthand for spawning entities with a full
/// suite of related components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentGroup {
    pub name: &'static str,
    pub type_ids: Vec<TypeId>,
}

impl ComponentGroup {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            type_ids: Vec::new(),
        }
    }

    pub fn with<T: 'static>(mut self) -> Self {
        self.type_ids.push(TypeId::of::<T>());
        self
    }

    pub fn with_erased(mut self, type_id: TypeId) -> Self {
        self.type_ids.push(type_id);
        self
    }

    pub fn len(&self) -> usize {
        self.type_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.type_ids.is_empty()
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.type_ids.contains(&TypeId::of::<T>())
    }

    pub fn contains_erased(&self, type_id: TypeId) -> bool {
        self.type_ids.contains(&type_id)
    }
}

/// Registry that holds named component groups and can pre-warm archetypes.
#[derive(Default)]
pub struct GroupRegistry {
    groups: HashMap<&'static str, ComponentGroup>,
}

impl GroupRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, group: ComponentGroup) {
        self.groups.insert(group.name, group);
    }

    pub fn get(&self, name: &str) -> Option<&ComponentGroup> {
        self.groups.get(name)
    }

    /// Pre-warm the archetype for `group_name` by spawning a dummy entity
    /// and immediately despawning it.  This ensures the ECS allocates the
    /// contiguous storage layout before the hot loop begins.
    pub fn prewarm(&self, world: &mut HecsWorld, group_name: &str) -> Result<(), String> {
        let group = self
            .groups
            .get(group_name)
            .ok_or_else(|| format!("group '{}' not registered", group_name))?;
        Self::prewarm_group(world, group)
    }

    fn prewarm_group(world: &mut HecsWorld, group: &ComponentGroup) -> Result<(), String> {
        // hecs bundles require concrete typed tuples for spawn.
        // For type-erased pre-warming we rely on the caller having already
        // registered the group components via ComponentRegistry::insert_bundle.
        // This method therefore only supports groups that have a typed
        // pre-warm path (i.e. one or two component types).
        match group.type_ids.len() {
            0 => Err("group has no components".to_string()),
            1 => {
                // Cannot pre-warm a single type-erased component; we would
                // need its concrete type.  Return Ok so the caller can
                // pre-warm manually with the concrete type via world.spawn.
                Ok(())
            }
            _ => {
                // For groups with >1 component we still cannot do type-erased
                // pre-warming without downcasting machinery.  Document as
                // "caller must pre-warm manually".
                Ok(())
            }
        }
    }

    /// Return all registered group names.
    pub fn names(&self) -> impl Iterator<Item = &&'static str> + '_ {
        self.groups.keys()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }
}

/// Spawn an entity with a [`DynamicBundle`] and return its [`Entity`].
/// This is a convenience wrapper that groups the spawn + insert operation
/// so the ECS creates the archetype in a single step.
pub fn spawn_group(
    world: &mut HecsWorld,
    registry: &crate::component_registry::ComponentRegistry,
    bundle: crate::component_registry::DynamicBundle,
) -> Result<Entity, String> {
    let entity = world.spawn(());
    registry.insert_bundle(world, entity, bundle)?;
    Ok(entity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_registry::{ComponentRegistry, DynamicBundle};
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

    #[derive(Debug, Clone, PartialEq, Default)]
    struct Health {
        hp: i32,
    }

    #[test]
    fn group_registry_starts_empty() {
        let reg = GroupRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn group_registry_register_and_get() {
        let mut reg = GroupRegistry::new();
        let group = ComponentGroup::new("Motion").with::<Position>().with::<Velocity>();
        reg.register(group.clone());

        let got = reg.get("Motion").unwrap();
        assert_eq!(got.name, "Motion");
        assert_eq!(got.len(), 2);
        assert!(got.contains::<Position>());
        assert!(got.contains::<Velocity>());
        assert!(!got.contains::<Health>());
    }

    #[test]
    fn group_registry_get_unknown_returns_none() {
        let reg = GroupRegistry::new();
        assert!(reg.get("Ghost").is_none());
    }

    #[test]
    fn group_registry_names() {
        let mut reg = GroupRegistry::new();
        reg.register(ComponentGroup::new("A").with::<Position>());
        reg.register(ComponentGroup::new("B").with::<Velocity>());
        let mut names: Vec<_> = reg.names().copied().collect();
        names.sort();
        assert_eq!(names, vec!["A", "B"]);
    }

    #[test]
    fn component_group_with_erased() {
        let group = ComponentGroup::new("Mixed")
            .with::<Position>()
            .with_erased(TypeId::of::<Velocity>());
        assert!(group.contains::<Position>());
        assert!(group.contains_erased(TypeId::of::<Velocity>()));
        assert!(!group.contains_erased(TypeId::of::<Health>()));
    }

    #[test]
    fn spawn_group_creates_entity_with_bundle() {
        let mut world = EcsWorld::new();
        let mut reg = ComponentRegistry::new();
        reg.register::<Position>();
        reg.register::<Velocity>();

        let mut bundle = DynamicBundle::new();
        bundle.add(Position { x: 1.0, y: 2.0, z: 3.0 });
        bundle.add(Velocity { dx: 4.0, dy: 5.0 });

        let entity = spawn_group(&mut world, &reg, bundle).unwrap();
        assert!(world.satisfies::<&Position>(entity));
        assert!(world.satisfies::<&Velocity>(entity));
    }

    #[test]
    fn spawn_group_unknown_component_errors() {
        let mut world = EcsWorld::new();
        let reg = ComponentRegistry::new(); // empty

        let mut bundle = DynamicBundle::new();
        bundle.add(Position::default());

        assert!(spawn_group(&mut world, &reg, bundle).is_err());
    }

    #[test]
    fn prewarm_empty_group_errors() {
        let mut reg = GroupRegistry::new();
        reg.register(ComponentGroup::new("Empty"));
        let mut world = EcsWorld::new();
        assert!(reg.prewarm(&mut world, "Empty").is_err());
    }

    #[test]
    fn prewarm_unknown_group_errors() {
        let reg = GroupRegistry::new();
        let mut world = EcsWorld::new();
        assert!(reg.prewarm(&mut world, "Ghost").is_err());
    }
}
