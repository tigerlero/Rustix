//! Scene graph: entity hierarchy with parent-child transforms.
//!
//! `Parent` and `Children` components link entities. A system can
//! propagate world transforms from root to leaf.

use hecs::Entity;
use rustix_core::math::{Vec3, Quat, Mat4};

/// A component that marks an entity as having a parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parent {
    pub entity: Entity,
}

impl Parent {
    pub fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

/// A component that stores an entity's children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Children {
    pub entities: Vec<Entity>,
}

impl Children {
    pub fn new() -> Self {
        Self { entities: Vec::new() }
    }

    pub fn with(entities: Vec<Entity>) -> Self {
        Self { entities }
    }

    pub fn add(&mut self, entity: Entity) {
        if !self.entities.contains(&entity) {
            self.entities.push(entity);
        }
    }

    pub fn remove(&mut self, entity: Entity) {
        self.entities.retain(|&e| e != entity);
    }
}

impl Default for Children {
    fn default() -> Self {
        Self::new()
    }
}

/// Local transform relative to the parent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl LocalTransform {
    pub fn new() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl Default for LocalTransform {
    fn default() -> Self {
        Self::new()
    }
}

/// Global world transform computed from the hierarchy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalTransform {
    pub matrix: Mat4,
}

impl GlobalTransform {
    pub fn new() -> Self {
        Self {
            matrix: Mat4::IDENTITY,
        }
    }

    pub fn from_matrix(matrix: Mat4) -> Self {
        Self { matrix }
    }

    pub fn translation(&self) -> Vec3 {
        self.matrix.w_axis.truncate()
    }
}

impl Default for GlobalTransform {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a topologically sorted list of entities from root to leaves.
/// Returns `(sorted_entities, roots)`.
pub fn compute_hierarchy_depth_first(world: &hecs::World) -> (Vec<Entity>, Vec<Entity>) {
    let mut sorted = Vec::new();
    let mut roots = Vec::new();
    let mut visited = std::collections::HashSet::new();

    for entity in world.query::<hecs::Entity>().iter() {
        if world.get::<&Parent>(entity).is_err() {
            roots.push(entity);
        }
    }

    fn visit(
        world: &hecs::World,
        entity: Entity,
        sorted: &mut Vec<Entity>,
        visited: &mut std::collections::HashSet<Entity>,
    ) {
        if !visited.insert(entity) {
            return;
        }
        sorted.push(entity);
        if let Ok(children) = world.get::<&Children>(entity) {
            for &child in &children.entities {
                visit(world, child, sorted, visited);
            }
        }
    }

    for &root in &roots {
        visit(world, root, &mut sorted, &mut visited);
    }

    (sorted, roots)
}

/// Propagate `LocalTransform` to `GlobalTransform` for all entities.
pub fn propagate_transforms(world: &mut hecs::World) {
    let (sorted, _) = compute_hierarchy_depth_first(world);

    let mut computed = std::collections::HashMap::with_capacity(sorted.len());

    for entity in &sorted {
        let local = world
            .get::<&LocalTransform>(*entity)
            .map(|t| *t)
            .unwrap_or_default();
        let local_matrix = local.to_matrix();

        let global_matrix = if let Ok(parent) = world.get::<&Parent>(*entity) {
            if let Some(&parent_matrix) = computed.get(&parent.entity) {
                parent_matrix * local_matrix
            } else {
                local_matrix
            }
        } else {
            local_matrix
        };

        computed.insert(*entity, global_matrix);
    }

    for (entity, matrix) in computed {
        if world.get::<&mut GlobalTransform>(entity).is_ok() {
            world.get::<&mut GlobalTransform>(entity).unwrap().matrix = matrix;
        } else {
            let _ = world.insert(entity, (GlobalTransform::from_matrix(matrix),));
        }
    }
}
