//! Tests for specialization constants.

use crate::spec_constants::SpecConstantMap;

#[test]
fn spec_constant_map_new() {
    let map = SpecConstantMap::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
}

#[test]
fn spec_constant_map_default() {
    let map: SpecConstantMap = Default::default();
    assert!(map.is_empty());
}

#[test]
fn spec_constant_map_set_and_get() {
    let mut map = SpecConstantMap::new();
    map.set(0, 4u32);
    assert_eq!(map.get(0), Some(4));
    assert_eq!(map.len(), 1);
}

#[test]
fn spec_constant_map_overwrite() {
    let mut map = SpecConstantMap::new();
    map.set(0, 1u32);
    map.set(0, 2u32);
    assert_eq!(map.get(0), Some(2));
    assert_eq!(map.len(), 1);
}

#[test]
fn spec_constant_map_multiple_entries() {
    let mut map = SpecConstantMap::new();
    map.set(0, 1u32).set(1, 2u32).set(2, 3u32);
    assert_eq!(map.get(0), Some(1));
    assert_eq!(map.get(1), Some(2));
    assert_eq!(map.get(2), Some(3));
    assert_eq!(map.len(), 3);
}

#[test]
fn spec_constant_map_get_missing() {
    let map = SpecConstantMap::new();
    assert_eq!(map.get(99), None);
}

#[test]
fn spec_constant_map_build() {
    let mut map = SpecConstantMap::new();
    map.set(0, 1u32).set(1, 2u32);
    let (entries, data) = map.build();
    assert_eq!(entries.len(), 2);
    assert_eq!(data.len(), 8);
}
