//! Tests for navigation mesh.

use rustix_core::components::Transform;
use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;
use rustix_physics::{BodyType, Collider, ColliderShape, RigidBody};
use crate::nav::{NavMesh, NavMeshGenerator, NavMeshSource, NavTriangle};

#[test]
fn test_triangle_contains() {
    let tri = NavTriangle::new(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 2.0),
    );
    assert!(tri.contains_point(Vec3::new(0.5, 0.0, 0.5)));
    assert!(!tri.contains_point(Vec3::new(1.5, 0.0, 1.5)));
}

#[test]
fn test_navmesh_pathfinding() {
    let mut nav = NavMesh::new();
    // Create two adjacent triangles
    nav.add_triangle(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
    nav.add_triangle(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 1.0));

    let pf = nav.to_pathfinder();
    let path = pf.find_path(0, 1);
    assert!(path.is_some());
}

#[test]
fn test_query_counts_entities() {
    let mut world = EcsWorld::new();
    world.spawn((
        RigidBody {
            body_type: BodyType::Static,
            ..Default::default()
        },
        Collider {
            shape: ColliderShape::Box { half_extents: Vec3::new(5.0, 0.5, 5.0) },
            ..Default::default()
        },
        Transform {
            translation: Vec3::new(0.0, 0.0, 0.0),
            ..Default::default()
        },
    ));

    let count = world.query::<(&RigidBody, &Collider, &Transform)>().iter().count();
    assert_eq!(count, 1, "query should find exactly 1 entity with all 3 components");
}

#[test]
fn test_navmesh_generator_from_box_colliders() {
    let mut world = EcsWorld::new();

    // Static floor box at y=0
    let _floor = world.spawn((
        RigidBody {
            body_type: BodyType::Static,
            ..Default::default()
        },
        Collider {
            shape: ColliderShape::Box { half_extents: Vec3::new(5.0, 0.5, 5.0) },
            ..Default::default()
        },
        Transform {
            translation: Vec3::new(0.0, 0.0, 0.0),
            ..Default::default()
        },
    ));

    // Dynamic box (should be ignored)
    let _dynamic = world.spawn((
        RigidBody {
            body_type: BodyType::Dynamic,
            ..Default::default()
        },
        Collider {
            shape: ColliderShape::Box { half_extents: Vec3::new(1.0, 1.0, 1.0) },
            ..Default::default()
        },
        Transform {
            translation: Vec3::new(10.0, 0.0, 0.0),
            ..Default::default()
        },
    ));

    let mut gen = NavMeshGenerator::new();
    gen.from_colliders(&world);
    let nav = gen.build();

    // Should have 2 triangles from the static box top face
    assert_eq!(nav.triangles.len(), 2, "expected 2 triangles from static box top face, got {}", nav.triangles.len());
}

#[test]
fn test_navmesh_generator_from_sources() {
    let mut world = EcsWorld::new();

    let source = NavMeshSource::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
        vec![[0, 1, 2]],
    );

    world.spawn((
        source,
        Transform {
            translation: Vec3::new(5.0, 0.0, 0.0),
            ..Default::default()
        },
    ));

    let mut gen = NavMeshGenerator::new();
    gen.from_sources(&world);
    let nav = gen.build();

    assert_eq!(nav.triangles.len(), 1);
    // Should be translated by (5, 0, 0)
    assert!((nav.triangles[0].vertices[0].x - 5.0).abs() < 0.001);
}
