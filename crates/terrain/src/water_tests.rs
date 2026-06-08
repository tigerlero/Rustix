//! Tests for water plane and shoreline detection.

use crate::water::{find_shoreline, find_water_body, water_stats};
use crate::Heightmap;

#[test]
fn find_shoreline_finds_cells_at_level() {
    let mut hm = Heightmap::flat(5, 5, 0.0);
    hm.set(2, 2, 1.0);
    hm.set(2, 3, 1.0);
    let shore = find_shoreline(&hm, 1.0, 0.1);
    assert!(shore.contains(&(2, 2)), "should find cell at water level");
    assert!(shore.contains(&(2, 3)), "should find cell at water level");
}

#[test]
fn find_shoreline_empty_when_no_match() {
    let hm = Heightmap::flat(5, 5, 0.0);
    let shore = find_shoreline(&hm, 10.0, 0.1);
    assert!(shore.is_empty());
}

#[test]
fn find_water_body_flood_fill() {
    let mut hm = Heightmap::flat(5, 5, 0.0);
    // Create a depression in the center
    hm.set(1, 1, -2.0);
    hm.set(2, 1, -2.0);
    hm.set(1, 2, -2.0);
    hm.set(2, 2, -2.0);

    let body = find_water_body(&hm, -1.0, 2, 2);
    assert_eq!(body.len(), 4, "should find 4 connected cells below water level");
}

#[test]
fn find_water_body_respects_bounds() {
    let hm = Heightmap::flat(5, 5, 0.0);
    let body = find_water_body(&hm, -1.0, 0, 0);
    assert!(body.is_empty(), "no cells below water level");
}

#[test]
fn water_stats_max_depth() {
    let mut hm = Heightmap::flat(5, 5, 0.0);
    hm.set(2, 2, -3.0);
    let (_, max_depth) = water_stats(&hm, 0.0);
    assert!((max_depth - 3.0).abs() < 1e-4, "max depth should be 3.0, got {}", max_depth);
}

#[test]
fn water_stats_shoreline_count() {
    let mut hm = Heightmap::flat(5, 5, 0.0);
    // One cell below water with a dry neighbor
    hm.set(2, 2, -1.0);
    let (shore_len, _) = water_stats(&hm, 0.0);
    assert_eq!(shore_len, 1, "one underwater cell touching dry land");
}
