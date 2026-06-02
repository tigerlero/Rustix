use std::collections::{HashMap, HashSet, VecDeque};

use glam::Mat4;
use hecs::{Entity, World};

use crate::components::{LocalToWorld, Parent, Transform};

/// Transform hierarchy system.
///
/// Computes world-space `LocalToWorld` matrices from `Transform` + `Parent`
/// components.  Operates in one breadth-first pass from roots so each child
/// is computed after its parent.
///
/// # Example
///
/// ```rust
/// use glam::Mat4;
/// use rustix_core::transform_hierarchy::Hierarchy;
/// use rustix_core::components::{Transform, Parent, LocalToWorld};
///
/// let mut world = hecs::World::new();
/// let root = world.spawn((Transform::default(), Parent::default()));
/// let child = world.spawn((Transform::default(), Parent(Some(root))));
///
/// Hierarchy::update_local_to_world(&mut world);
///
/// let ltw = world.get::<&LocalToWorld>(child).unwrap();
/// assert_eq!(ltw.matrix, Mat4::IDENTITY);
/// ```
pub struct Hierarchy;

impl Hierarchy {
    /// Compute `LocalToWorld` for every entity that has a `Transform`.
    ///
    /// Entities without a `Parent` are treated as roots.  Entities with
    /// `Parent(None)` are also roots.  The world matrix is:
    ///
    ///   `world = parent_world * local`
    ///
    /// If an entity already has a `LocalToWorld` it is overwritten;
    /// otherwise one is inserted.
    pub fn update_local_to_world(world: &mut World) {
        // Collect all entities with Transform + optional Parent
        let mut parent_of: HashMap<Entity, Entity> = HashMap::new();
        let mut roots: Vec<Entity> = Vec::new();

        for (entity, parent) in world.query::<(Entity, &Parent)>().iter() {
            match parent.0 {
                Some(p) => {
                    parent_of.insert(entity, p);
                }
                None => {
                    roots.push(entity);
                }
            }
        }

        // Entities with Transform but no Parent component are also roots
        for (entity, _transform) in world.query::<(Entity, &Transform)>().iter() {
            if !parent_of.contains_key(&entity) && !roots.contains(&entity) {
                roots.push(entity);
            }
        }

        // BFS from roots
        let mut queue: VecDeque<(Entity, Mat4)> = VecDeque::new();
        for root in &roots {
            let local = match world.get::<&Transform>(*root) {
                Ok(t) => t.matrix(),
                Err(_) => continue,
            };
            Self::write_ltw(world, *root, local);
            queue.push_back((*root, local));
        }

        // Build reverse map: parent -> children
        let mut children_of: HashMap<Entity, Vec<Entity>> = HashMap::new();
        for (child, parent) in &parent_of {
            children_of.entry(*parent).or_default().push(*child);
        }

        while let Some((parent_entity, parent_world)) = queue.pop_front() {
            if let Some(children) = children_of.get(&parent_entity) {
                for child in children {
                    let local = match world.get::<&Transform>(*child) {
                        Ok(t) => t.matrix(),
                        Err(_) => continue,
                    };
                    let world_matrix = parent_world * local;
                    Self::write_ltw(world, *child, world_matrix);
                    queue.push_back((*child, world_matrix));
                }
            }
        }
    }

    /// Set `entity`'s parent to `new_parent`.  Returns `Err` if the
    /// operation would create a cycle.
    pub fn set_parent(
        world: &mut World,
        entity: Entity,
        new_parent: Option<Entity>,
    ) -> Result<(), HierarchyError> {
        if let Some(p) = new_parent {
            if p == entity {
                return Err(HierarchyError::SelfParent);
            }
            if Self::would_create_cycle(world, entity, p) {
                return Err(HierarchyError::CycleDetected);
            }
        }
        let _ = world.insert(entity, (Parent(new_parent),));
        Ok(())
    }

    /// Check whether making `descendant` a child of `ancestor` would
    /// create a cycle.
    fn would_create_cycle(world: &World, descendant: Entity, ancestor: Entity) -> bool {
        let mut current = Some(ancestor);
        let mut depth = 0usize;
        while let Some(e) = current {
            if depth > 1024 {
                return true; // treat excessive depth as cycle
            }
            if e == descendant {
                return true;
            }
            current = world
                .get::<&Parent>(e)
                .ok()
                .and_then(|p| p.0);
            depth += 1;
        }
        false
    }

    /// Return an iterator over entities in topological order (roots first).
    pub fn topo_order(world: &World) -> Vec<Entity> {
        let mut parent_of: HashMap<Entity, Entity> = HashMap::new();
        let mut roots: Vec<Entity> = Vec::new();

        for (entity, parent) in world.query::<(Entity, &Parent)>().iter() {
            match parent.0 {
                Some(p) => {
                    parent_of.insert(entity, p);
                }
                None => roots.push(entity),
            }
        }

        // Also include entities with Transform but no Parent
        for (entity, _transform) in world.query::<(Entity, &Transform)>().iter() {
            if !parent_of.contains_key(&entity) && !roots.contains(&entity) {
                roots.push(entity);
            }
        }

        let mut children_of: HashMap<Entity, Vec<Entity>> = HashMap::new();
        for (child, parent) in &parent_of {
            children_of.entry(*parent).or_default().push(*child);
        }

        let mut order = Vec::new();
        let mut queue: VecDeque<Entity> = VecDeque::from_iter(roots);
        let mut visited: HashSet<Entity> = HashSet::new();

        while let Some(e) = queue.pop_front() {
            if !visited.insert(e) {
                continue;
            }
            order.push(e);
            if let Some(children) = children_of.get(&e) {
                for c in children {
                    queue.push_back(*c);
                }
            }
        }
        order
    }

    fn write_ltw(world: &mut World, entity: Entity, matrix: Mat4) {
        if world.get::<&mut LocalToWorld>(entity).is_ok() {
            let _ = world.insert(entity, (LocalToWorld { matrix },));
        } else {
            let _ = world.insert(entity, (LocalToWorld { matrix },));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HierarchyError {
    SelfParent,
    CycleDetected,
}

impl std::fmt::Display for HierarchyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HierarchyError::SelfParent => write!(f, "entity cannot be its own parent"),
            HierarchyError::CycleDetected => write!(f, "parent change would create a cycle"),
        }
    }
}

impl std::error::Error for HierarchyError {}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Quat, Vec3};

    #[test]
    fn hierarchy_root_only() {
        let mut world = World::new();
        let root = world.spawn((
            Transform {
                translation: Vec3::new(1.0, 2.0, 3.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            Parent(None),
        ));

        Hierarchy::update_local_to_world(&mut world);

        let ltw = world.get::<&LocalToWorld>(root).unwrap();
        let (_, _, pos) = ltw.matrix.to_scale_rotation_translation();
        assert_eq!(pos, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn hierarchy_child_inherits_parent() {
        let mut world = World::new();
        let root = world.spawn((
            Transform {
                translation: Vec3::new(1.0, 0.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            Parent(None),
        ));
        let child = world.spawn((
            Transform {
                translation: Vec3::new(0.0, 2.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            Parent(Some(root)),
        ));

        Hierarchy::update_local_to_world(&mut world);

        let ltw = world.get::<&LocalToWorld>(child).unwrap();
        let (_, _, pos) = ltw.matrix.to_scale_rotation_translation();
        assert_eq!(pos, Vec3::new(1.0, 2.0, 0.0));
    }

    #[test]
    fn hierarchy_grandchild() {
        let mut world = World::new();
        let root = world.spawn((
            Transform {
                translation: Vec3::new(1.0, 0.0, 0.0),
                ..Default::default()
            },
            Parent(None),
        ));
        let child = world.spawn((
            Transform {
                translation: Vec3::new(0.0, 2.0, 0.0),
                ..Default::default()
            },
            Parent(Some(root)),
        ));
        let grandchild = world.spawn((
            Transform {
                translation: Vec3::new(0.0, 0.0, 3.0),
                ..Default::default()
            },
            Parent(Some(child)),
        ));

        Hierarchy::update_local_to_world(&mut world);

        let ltw = world.get::<&LocalToWorld>(grandchild).unwrap();
        let (_, _, pos) = ltw.matrix.to_scale_rotation_translation();
        assert_eq!(pos, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn hierarchy_scale_composition() {
        let mut world = World::new();
        let root = world.spawn((
            Transform {
                scale: Vec3::splat(2.0),
                ..Default::default()
            },
            Parent(None),
        ));
        let child = world.spawn((
            Transform {
                scale: Vec3::splat(3.0),
                ..Default::default()
            },
            Parent(Some(root)),
        ));

        Hierarchy::update_local_to_world(&mut world);

        let ltw = world.get::<&LocalToWorld>(child).unwrap();
        let (scale, _, _) = ltw.matrix.to_scale_rotation_translation();
        assert!((scale - Vec3::splat(6.0)).length() < 0.001);
    }

    #[test]
    fn hierarchy_rotation_composition() {
        use glam::EulerRot;
        let mut world = World::new();
        let root = world.spawn((
            Transform {
                rotation: Quat::from_euler(EulerRot::XYZ, 90f32.to_radians(), 0.0, 0.0),
                ..Default::default()
            },
            Parent(None),
        ));
        let child = world.spawn((
            Transform {
                rotation: Quat::from_euler(EulerRot::XYZ, 0.0, 90f32.to_radians(), 0.0),
                ..Default::default()
            },
            Parent(Some(root)),
        ));

        Hierarchy::update_local_to_world(&mut world);

        let ltw = world.get::<&LocalToWorld>(child).unwrap();
        let (_, rot, _) = ltw.matrix.to_scale_rotation_translation();
        // Combined rotation should be non-identity
        assert_ne!(rot, Quat::IDENTITY);
    }

    #[test]
    fn hierarchy_set_parent_ok() {
        let mut world = World::new();
        let a = world.spawn((Transform::default(), Parent(None)));
        let b = world.spawn((Transform::default(), Parent(None)));

        Hierarchy::set_parent(&mut world, b, Some(a)).unwrap();
        let p = world.get::<&Parent>(b).unwrap();
        assert_eq!(p.0, Some(a));
    }

    #[test]
    fn hierarchy_set_parent_self_rejected() {
        let mut world = World::new();
        let a = world.spawn((Transform::default(), Parent(None)));

        let err = Hierarchy::set_parent(&mut world, a, Some(a)).unwrap_err();
        assert_eq!(err, HierarchyError::SelfParent);
    }

    #[test]
    fn hierarchy_set_parent_cycle_detected() {
        let mut world = World::new();
        let a = world.spawn((Transform::default(), Parent(None)));
        let b = world.spawn((Transform::default(), Parent(Some(a))));

        // Making a child of b would create a cycle: a → b → a
        let err = Hierarchy::set_parent(&mut world, a, Some(b)).unwrap_err();
        assert_eq!(err, HierarchyError::CycleDetected);
    }

    #[test]
    fn hierarchy_topo_order() {
        let mut world = World::new();
        let a = world.spawn((Transform::default(), Parent(None)));
        let b = world.spawn((Transform::default(), Parent(Some(a))));
        let c = world.spawn((Transform::default(), Parent(Some(a))));
        let d = world.spawn((Transform::default(), Parent(Some(b))));

        let order = Hierarchy::topo_order(&world);
        let a_idx = order.iter().position(|&e| e == a).unwrap();
        let b_idx = order.iter().position(|&e| e == b).unwrap();
        let c_idx = order.iter().position(|&e| e == c).unwrap();
        let d_idx = order.iter().position(|&e| e == d).unwrap();

        assert!(a_idx < b_idx);
        assert!(a_idx < c_idx);
        assert!(b_idx < d_idx);
    }

    #[test]
    fn hierarchy_missing_parent_component_treated_as_root() {
        let mut world = World::new();
        let root = world.spawn((Transform {
            translation: Vec3::new(5.0, 0.0, 0.0),
            ..Default::default()
        },));

        Hierarchy::update_local_to_world(&mut world);

        let ltw = world.get::<&LocalToWorld>(root).unwrap();
        let (_, _, pos) = ltw.matrix.to_scale_rotation_translation();
        assert_eq!(pos, Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn hierarchy_update_overwrites_existing_ltw() {
        let mut world = World::new();
        let e = world.spawn((
            Transform {
                translation: Vec3::new(1.0, 0.0, 0.0),
                ..Default::default()
            },
            LocalToWorld { matrix: Mat4::IDENTITY },
        ));

        Hierarchy::update_local_to_world(&mut world);

        let ltw = world.get::<&LocalToWorld>(e).unwrap();
        let (_, _, pos) = ltw.matrix.to_scale_rotation_translation();
        assert_eq!(pos, Vec3::new(1.0, 0.0, 0.0));
    }
}
