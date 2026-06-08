//! Tests for scene graph hierarchy and transform propagation.

use hecs::World;
use rustix_core::math::{Vec3, Quat, Mat4};
use crate::scene_graph::{Parent, Children, LocalTransform, GlobalTransform, compute_hierarchy_depth_first, propagate_transforms};

fn setup_world() -> (World, hecs::Entity, hecs::Entity, hecs::Entity) {
    let mut world = World::new();
    let root = world.spawn((LocalTransform::default(), GlobalTransform::default()));
    let child = world.spawn((LocalTransform::default(), GlobalTransform::default(), Parent::new(root)));
    let grandchild = world.spawn((LocalTransform::default(), GlobalTransform::default(), Parent::new(child)));

    // Set up children components on root and child
    world.insert(root, (Children::with(vec![child]),));
    world.insert(child, (Children::with(vec![grandchild]),));

    (world, root, child, grandchild)
}

#[test]
fn local_transform_default() {
    let t = LocalTransform::default();
    assert_eq!(t.translation, Vec3::ZERO);
    assert_eq!(t.rotation, Quat::IDENTITY);
    assert_eq!(t.scale, Vec3::ONE);
}

#[test]
fn local_transform_to_matrix_identity() {
    let t = LocalTransform::default();
    assert_eq!(t.to_matrix(), Mat4::IDENTITY);
}

#[test]
fn global_transform_default() {
    let t = GlobalTransform::default();
    assert_eq!(t.matrix, Mat4::IDENTITY);
}

#[test]
fn global_transform_translation() {
    let mut t = GlobalTransform::default();
    t.matrix = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(t.translation(), Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn children_add() {
    let mut world = World::new();
    let e = world.spawn(());
    let mut children = Children::new();
    children.add(e);
    assert_eq!(children.entities.len(), 1);
}

#[test]
fn children_add_deduplicates() {
    let mut world = World::new();
    let e = world.spawn(());
    let mut children = Children::new();
    children.add(e);
    children.add(e);
    assert_eq!(children.entities.len(), 1);
}

#[test]
fn children_remove() {
    let mut world = World::new();
    let e = world.spawn(());
    let mut children = Children::with(vec![e]);
    children.remove(e);
    assert!(children.entities.is_empty());
}

#[test]
fn compute_hierarchy_finds_roots() {
    let (world, root, _child, _grandchild) = setup_world();
    let (_sorted, roots) = compute_hierarchy_depth_first(&world);
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0], root);
}

#[test]
fn compute_hierarchy_order() {
    let (world, root, child, grandchild) = setup_world();
    let (sorted, _roots) = compute_hierarchy_depth_first(&world);
    let root_idx = sorted.iter().position(|&e| e == root).unwrap();
    let child_idx = sorted.iter().position(|&e| e == child).unwrap();
    let grandchild_idx = sorted.iter().position(|&e| e == grandchild).unwrap();
    assert!(root_idx < child_idx);
    assert!(child_idx < grandchild_idx);
}

#[test]
fn propagate_transforms_identity() {
    let (mut world, root, _child, _grandchild) = setup_world();
    propagate_transforms(&mut world);
    let gt = world.get::<&GlobalTransform>(root).unwrap();
    assert_eq!(gt.matrix, Mat4::IDENTITY);
}

#[test]
fn propagate_transforms_child_inherits_parent() {
    let mut world = World::new();
    let root = world.spawn((
        LocalTransform { translation: Vec3::new(1.0, 0.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::ONE },
        GlobalTransform::default(),
    ));
    let child = world.spawn((
        LocalTransform { translation: Vec3::new(0.0, 2.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::ONE },
        GlobalTransform::default(),
        Parent::new(root),
    ));
    world.insert(root, (Children::with(vec![child]),));

    propagate_transforms(&mut world);

    let gt = world.get::<&GlobalTransform>(child).unwrap();
    assert_eq!(gt.translation(), Vec3::new(1.0, 2.0, 0.0));
}

#[test]
fn propagate_transforms_grandchild() {
    let (mut world, root, child, grandchild) = setup_world();

    // Set local transforms
    world.get::<&mut LocalTransform>(root).unwrap().translation = Vec3::new(1.0, 0.0, 0.0);
    world.get::<&mut LocalTransform>(child).unwrap().translation = Vec3::new(0.0, 2.0, 0.0);
    world.get::<&mut LocalTransform>(grandchild).unwrap().translation = Vec3::new(0.0, 0.0, 3.0);

    propagate_transforms(&mut world);

    let gt = world.get::<&GlobalTransform>(grandchild).unwrap();
    assert_eq!(gt.translation(), Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn propagate_transforms_no_parent_uses_local() {
    let mut world = World::new();
    let e = world.spawn((
        LocalTransform { translation: Vec3::new(5.0, 0.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::ONE },
    ));

    propagate_transforms(&mut world);

    let gt = world.get::<&GlobalTransform>(e).unwrap();
    assert_eq!(gt.translation(), Vec3::new(5.0, 0.0, 0.0));
}
