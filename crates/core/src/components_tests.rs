//! Tests for core ECS components.

use glam::{Mat4, Quat, Vec3};
use crate::components::{Transform, Parent, LocalToWorld, ScriptComponent};

#[test]
fn transform_default() {
    let t = Transform::default();
    assert_eq!(t.translation, Vec3::ZERO);
    assert_eq!(t.rotation, Quat::IDENTITY);
    assert_eq!(t.scale, Vec3::ONE);
}

#[test]
fn transform_from_translation() {
    let t = Transform::from_translation(Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(t.translation, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(t.rotation, Quat::IDENTITY);
    assert_eq!(t.scale, Vec3::ONE);
}

#[test]
fn transform_from_translation_rotation_scale() {
    let t = Transform::from_translation_rotation_scale(Vec3::new(1.0, 0.0, 0.0), 0.0, 2.0);
    assert_eq!(t.translation, Vec3::new(1.0, 0.0, 0.0));
    assert_eq!(t.scale, Vec3::splat(2.0));
}

#[test]
fn transform_matrix_identity() {
    let t = Transform::default();
    assert_eq!(t.matrix(), Mat4::IDENTITY);
}

#[test]
fn transform_matrix_translation() {
    let t = Transform::from_translation(Vec3::new(5.0, 0.0, 0.0));
    let m = t.matrix();
    let (_, _, pos) = m.to_scale_rotation_translation();
    assert_eq!(pos, Vec3::new(5.0, 0.0, 0.0));
}

#[test]
fn transform_matrix_scale() {
    let mut t = Transform::default();
    t.scale = Vec3::splat(3.0);
    let m = t.matrix();
    let (scale, _, _) = m.to_scale_rotation_translation();
    assert_eq!(scale, Vec3::splat(3.0));
}

#[test]
fn parent_default_is_none() {
    let p = Parent::default();
    assert_eq!(p.0, None);
}

#[test]
fn local_to_world_default_is_identity() {
    let ltw = LocalToWorld::default();
    assert_eq!(ltw.matrix, Mat4::IDENTITY);
}

#[test]
fn script_component_default() {
    let sc = ScriptComponent::default();
    assert!(sc.source.is_empty());
    assert!(!sc.enabled);
}
