//! Tests for spatial hash grid.

use hecs::World;
use rustix_core::math::Vec3;
use crate::spatial::SpatialHash;

fn make_entities(count: usize) -> (World, Vec<hecs::Entity>) {
    let mut world = World::new();
    let entities: Vec<_> = (0..count).map(|_| world.spawn(())).collect();
    (world, entities)
}

#[test]
fn spatial_new_is_empty() {
    let spatial = SpatialHash::new(1.0);
    let results = spatial.query_cell(Vec3::ZERO);
    assert!(results.is_empty());
}

#[test]
fn spatial_insert_and_query() {
    let mut spatial = SpatialHash::new(1.0);
    let (_world, entities) = make_entities(1);
    let e = entities[0];
    spatial.insert(e, Vec3::new(0.5, 0.5, 0.5));
    let results = spatial.query_cell(Vec3::new(0.5, 0.5, 0.5));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], e);
}

#[test]
fn spatial_insert_multiple_same_cell() {
    let mut spatial = SpatialHash::new(1.0);
    let (_world, entities) = make_entities(2);
    let e1 = entities[0];
    let e2 = entities[1];
    spatial.insert(e1, Vec3::new(0.5, 0.5, 0.5));
    spatial.insert(e2, Vec3::new(0.5, 0.5, 0.5));
    let results = spatial.query_cell(Vec3::new(0.5, 0.5, 0.5));
    assert_eq!(results.len(), 2);
}

#[test]
fn spatial_different_cells() {
    let mut spatial = SpatialHash::new(1.0);
    let (_world, entities) = make_entities(2);
    let e1 = entities[0];
    let e2 = entities[1];
    spatial.insert(e1, Vec3::new(0.5, 0.5, 0.5));
    spatial.insert(e2, Vec3::new(1.5, 0.5, 0.5));
    assert_eq!(spatial.query_cell(Vec3::new(0.5, 0.5, 0.5)).len(), 1);
    assert_eq!(spatial.query_cell(Vec3::new(1.5, 0.5, 0.5)).len(), 1);
}

#[test]
fn spatial_remove() {
    let mut spatial = SpatialHash::new(1.0);
    let (_world, entities) = make_entities(1);
    let e = entities[0];
    spatial.insert(e, Vec3::new(0.5, 0.5, 0.5));
    spatial.remove(e, Vec3::new(0.5, 0.5, 0.5));
    assert!(spatial.query_cell(Vec3::new(0.5, 0.5, 0.5)).is_empty());
}

#[test]
fn spatial_update_moves_entity() {
    let mut spatial = SpatialHash::new(1.0);
    let (_world, entities) = make_entities(1);
    let e = entities[0];
    spatial.insert(e, Vec3::new(0.5, 0.5, 0.5));
    spatial.update(e, Vec3::new(0.5, 0.5, 0.5), Vec3::new(1.5, 0.5, 0.5));
    assert!(spatial.query_cell(Vec3::new(0.5, 0.5, 0.5)).is_empty());
    assert_eq!(spatial.query_cell(Vec3::new(1.5, 0.5, 0.5)).len(), 1);
}

#[test]
fn spatial_update_same_cell_no_op() {
    let mut spatial = SpatialHash::new(1.0);
    let (_world, entities) = make_entities(1);
    let e = entities[0];
    spatial.insert(e, Vec3::new(0.5, 0.5, 0.5));
    spatial.update(e, Vec3::new(0.5, 0.5, 0.5), Vec3::new(0.6, 0.6, 0.6));
    assert_eq!(spatial.query_cell(Vec3::new(0.5, 0.5, 0.5)).len(), 1);
}

#[test]
fn spatial_query_sphere_finds_nearby() {
    let mut spatial = SpatialHash::new(1.0);
    let (_world, entities) = make_entities(1);
    let e = entities[0];
    spatial.insert(e, Vec3::new(0.5, 0.5, 0.5));
    let results = spatial.query_sphere(Vec3::ZERO, 2.0);
    assert!(results.contains(&e));
}

#[test]
fn spatial_query_sphere_excludes_far() {
    let mut spatial = SpatialHash::new(1.0);
    let (_world, entities) = make_entities(1);
    let e = entities[0];
    spatial.insert(e, Vec3::new(10.0, 10.0, 10.0));
    let results = spatial.query_sphere(Vec3::ZERO, 1.0);
    assert!(!results.contains(&e));
}

#[test]
fn spatial_clear() {
    let mut spatial = SpatialHash::new(1.0);
    let (_world, entities) = make_entities(1);
    let e = entities[0];
    spatial.insert(e, Vec3::new(0.5, 0.5, 0.5));
    spatial.clear();
    assert!(spatial.query_cell(Vec3::new(0.5, 0.5, 0.5)).is_empty());
}
