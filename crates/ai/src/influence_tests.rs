//! Tests for influence map grid operations.

use crate::influence::InfluenceMap;

#[test]
fn influence_new_is_zeroed() {
    let map = InfluenceMap::new(5, 5, 1.0, [0.0, 0.0]);
    for y in 0..5 {
        for x in 0..5 {
            assert_eq!(map.get(x, y), 0.0);
        }
    }
}

#[test]
fn influence_set_and_get() {
    let mut map = InfluenceMap::new(5, 5, 1.0, [0.0, 0.0]);
    map.set(2, 3, 7.0);
    assert_eq!(map.get(2, 3), 7.0);
}

#[test]
fn influence_add_accumulates() {
    let mut map = InfluenceMap::new(5, 5, 1.0, [0.0, 0.0]);
    map.set(1, 1, 3.0);
    map.add(1, 1, 4.0);
    assert_eq!(map.get(1, 1), 7.0);
}

#[test]
fn influence_out_of_bounds_returns_zero() {
    let map = InfluenceMap::new(5, 5, 1.0, [0.0, 0.0]);
    assert_eq!(map.get(10, 10), 0.0);
}

#[test]
fn influence_clear_zeros_all() {
    let mut map = InfluenceMap::new(5, 5, 1.0, [0.0, 0.0]);
    map.set(2, 2, 10.0);
    map.clear();
    assert_eq!(map.get(2, 2), 0.0);
}

#[test]
fn influence_world_to_grid_center() {
    let map = InfluenceMap::new(10, 10, 1.0, [0.0, 0.0]);
    assert_eq!(map.world_to_grid(0.5, 0.5), (0, 0));
}

#[test]
fn influence_world_to_grid_clamps() {
    let map = InfluenceMap::new(5, 5, 1.0, [0.0, 0.0]);
    assert_eq!(map.world_to_grid(100.0, 100.0), (4, 4));
    assert_eq!(map.world_to_grid(-100.0, -100.0), (0, 0));
}

#[test]
fn influence_grid_to_world_roundtrip() {
    let map = InfluenceMap::new(10, 10, 2.0, [0.0, 0.0]);
    let (wx, wy) = map.grid_to_world(3, 4);
    let (gx, gy) = map.world_to_grid(wx, wy);
    assert_eq!(gx, 3);
    assert_eq!(gy, 4);
}

#[test]
fn influence_stamp_increases_center() {
    let mut map = InfluenceMap::new(10, 10, 1.0, [0.0, 0.0]);
    map.stamp_influence(5.0, 5.0, 10.0, 2.0);
    let center = map.get(5, 5);
    assert!(center > 0.0, "center should have positive influence");
}

#[test]
fn influence_highest_after_stamp() {
    let mut map = InfluenceMap::new(10, 10, 1.0, [0.0, 0.0]);
    map.stamp_influence(5.0, 5.0, 10.0, 2.0);
    let (x, y, v) = map.highest_cell().unwrap();
    assert_eq!(x, 5);
    assert_eq!(y, 5);
    assert!(v > 0.0);
}

#[test]
fn influence_lowest_empty_map() {
    let map = InfluenceMap::new(3, 3, 1.0, [0.0, 0.0]);
    let (x, y, v) = map.lowest_cell().unwrap();
    assert_eq!(v, 0.0);
    assert!(x < 3 && y < 3);
}

#[test]
fn influence_decay_reduces_values() {
    let mut map = InfluenceMap::new(3, 3, 1.0, [0.0, 0.0]);
    map.set(1, 1, 10.0);
    map.decay(0.5);
    assert_eq!(map.get(1, 1), 5.0);
}

#[test]
fn influence_clamp() {
    let mut map = InfluenceMap::new(3, 3, 1.0, [0.0, 0.0]);
    map.set(0, 0, 100.0);
    map.set(1, 1, -100.0);
    map.clamp(-10.0, 10.0);
    assert_eq!(map.get(0, 0), 10.0);
    assert_eq!(map.get(1, 1), -10.0);
}

#[test]
fn influence_add_map() {
    let mut a = InfluenceMap::new(3, 3, 1.0, [0.0, 0.0]);
    let mut b = InfluenceMap::new(3, 3, 1.0, [0.0, 0.0]);
    a.set(1, 1, 5.0);
    b.set(1, 1, 3.0);
    a.add_map(&b);
    assert_eq!(a.get(1, 1), 8.0);
}
