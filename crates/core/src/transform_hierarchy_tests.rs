//! Tests for transform hierarchy.

use glam::{Mat4, Quat, Vec3};
use hecs::World;
use crate::components::{LocalToWorld, Parent, Transform};
use crate::transform_hierarchy::{Hierarchy, HierarchyError};

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
