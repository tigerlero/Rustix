use hecs::{Entity, World as HecsWorld};
use std::any::TypeId;

use crate::component_registry::{ComponentRegistry, DynamicBundle};

/// A deferred ECS mutation that can be queued during system execution and
/// applied later in a single batch.
///
/// Command buffers solve the problem of modifying archetypes while iterating
/// over them.  Systems queue mutations during a tick; the engine applies all
/// commands after the tick’s systems have finished.
pub enum Command {
    /// Spawn a new entity and insert the given components.
    Spawn(DynamicBundle),
    /// Spawn a bare entity with no components.
    SpawnEmpty,
    /// Despawn an entity.
    Despawn(Entity),
    /// Insert a component bundle into an existing entity.
    InsertBundle(Entity, DynamicBundle),
    /// Remove a single component by its [`TypeId`].
    RemoveByTypeId(Entity, TypeId),
    /// Remove a single component by its registered short name.
    RemoveByName(Entity, String),
    /// Add a default-valued component by its registered short name.
    AddDefaultByName(Entity, String),
}

/// A growable list of deferred ECS operations.
///
/// Create a buffer, queue mutations with the `spawn`, `despawn`, `insert`,
/// `remove` and `add_default` helpers, then call [`CommandBuffer::apply`] at
/// a safe sync point to flush everything into the [`hecs::World`].
#[derive(Default)]
pub struct CommandBuffer {
    commands: Vec<Command>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a new entity with the components in `bundle`.
    pub fn spawn(&mut self, bundle: DynamicBundle) {
        self.commands.push(Command::Spawn(bundle));
    }

    /// Queue a bare entity with no components.
    pub fn spawn_empty(&mut self) {
        self.commands.push(Command::SpawnEmpty);
    }

    /// Queue despawn of `entity`.
    pub fn despawn(&mut self, entity: Entity) {
        self.commands.push(Command::Despawn(entity));
    }

    /// Queue insertion of a component bundle into `entity`.
    pub fn insert_bundle(&mut self, entity: Entity, bundle: DynamicBundle) {
        self.commands.push(Command::InsertBundle(entity, bundle));
    }

    /// Queue insertion of a single typed component into `entity`.
    pub fn insert_one<T: std::any::Any + Send + Sync + Clone>(&mut self, entity: Entity, component: T) {
        let mut bundle = DynamicBundle::new();
        bundle.add(component);
        self.commands.push(Command::InsertBundle(entity, bundle));
    }

    /// Queue removal of a component by its [`TypeId`].
    pub fn remove_by_type_id(&mut self, entity: Entity, type_id: TypeId) {
        self.commands.push(Command::RemoveByTypeId(entity, type_id));
    }

    /// Queue removal of a component by its registered short name.
    pub fn remove_by_name(&mut self, entity: Entity, name: impl Into<String>) {
        self.commands.push(Command::RemoveByName(entity, name.into()));
    }

    /// Queue addition of a default-valued component by its registered short name.
    pub fn add_default_by_name(&mut self, entity: Entity, name: impl Into<String>) {
        self.commands.push(Command::AddDefaultByName(entity, name.into()));
    }

    /// Apply every queued command to `world` using `registry` for type-erased
    /// dispatch.
    ///
    /// Commands are executed in the order they were queued.  The buffer is
    /// cleared on success.
    pub fn apply(
        &mut self,
        world: &mut HecsWorld,
        registry: &ComponentRegistry,
    ) -> Result<(), String> {
        for cmd in self.commands.drain(..) {
            match cmd {
                Command::Spawn(bundle) => {
                    let entity = world.spawn(());
                    registry.insert_bundle(world, entity, bundle)?;
                }
                Command::SpawnEmpty => {
                    world.spawn(());
                }
                Command::Despawn(entity) => {
                    let _ = world.despawn(entity);
                }
                Command::InsertBundle(entity, bundle) => {
                    registry.insert_bundle(world, entity, bundle)?;
                }
                Command::RemoveByTypeId(entity, type_id) => {
                    if let Some(info) = registry.get_by_type_id(type_id) {
                        let _ = info.remove_from_world(world, entity);
                    }
                }
                Command::RemoveByName(entity, name) => {
                    let _ = registry.remove_component_by_name(world, entity, &name);
                }
                Command::AddDefaultByName(entity, name) => {
                    registry.add_component_by_name(world, entity, &name)?;
                }
            }
        }
        Ok(())
    }

    /// Discard all queued commands without applying them.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Number of queued commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
