//! Tests for terrain erosion and splat modules.

use crate::*;
use crate::erosion::*;
use crate::splat::*;

// ---------- erosion.rs ----------

#[test]
fn thermal_erosion_params_default() {
    let p = ThermalErosionParams::default();
    assert_eq!(p.talus_angle, 1.0);
    assert_eq!(p.transport_rate, 0.5);
    assert_eq!(p.iterations, 20);
}

#[test]
fn thermal_erosion_flattens_peaks() {
    let mut hm = Heightmap::flat(5, 5, 0.0);
    hm.set(2, 2, 10.0); // peak in center
    let params = ThermalErosionParams {
        talus_angle: 1.0,
        transport_rate: 0.5,
        iterations: 10,
    };
    thermal_erosion(&mut hm, &params);
    // Peak should have eroded down
    assert!(hm.get(2, 2) < 10.0);
}

#[test]
fn thermal_erosion_no_change_on_flat() {
    let mut hm = Heightmap::flat(5, 5, 3.0);
    let params = ThermalErosionParams {
        talus_angle: 1.0,
        transport_rate: 0.5,
        iterations: 5,
    };
    thermal_erosion(&mut hm, &params);
    for z in 0..5 {
        for x in 0..5 {
            assert!((hm.get(x, z) - 3.0).abs() < 1e-3);
        }
    }
}

#[test]
fn hydraulic_erosion_params_default() {
    let p = HydraulicErosionParams::default();
    assert_eq!(p.rain_rate, 0.01);
    assert_eq!(p.solubility, 0.1);
    assert_eq!(p.evaporation, 0.05);
    assert_eq!(p.iterations, 20);
}

#[test]
fn hydraulic_erosion_changes_heightmap() {
    let mut hm = Heightmap::flat(5, 5, 5.0);
    hm.set(2, 2, 10.0); // peak
    let params = HydraulicErosionParams {
        rain_rate: 0.05,
        solubility: 0.5,
        evaporation: 0.1,
        iterations: 5,
    };
    let before = hm.get(2, 2);
    hydraulic_erosion(&mut hm, &params);
    // Should have dissolved some material
    assert!(hm.get(2, 2) < before);
}

#[test]
fn hydraulic_erosion_reduces_peaks() {
    let mut hm = Heightmap::flat(7, 7, 0.0);
    hm.set(3, 3, 20.0);
    let params = HydraulicErosionParams {
        rain_rate: 0.1,
        solubility: 1.0,
        evaporation: 0.1,
        iterations: 10,
    };
    hydraulic_erosion(&mut hm, &params);
    assert!(hm.get(3, 3) < 20.0);
}

// ---------- splat.rs ----------

#[test]
fn terrain_layer_new() {
    let layer = TerrainLayer::new("grass");
    assert_eq!(layer.name, "grass");
    assert_eq!(layer.min_height, f32::NEG_INFINITY);
    assert_eq!(layer.max_height, f32::INFINITY);
    assert_eq!(layer.min_slope, 0.0);
    assert_eq!(layer.max_slope, 1.0);
    assert_eq!(layer.base_weight, 1.0);
}

#[test]
fn terrain_layer_builder() {
    let layer = TerrainLayer::new("rock")
        .height_range(10.0, 50.0)
        .slope_range(0.3, 0.9)
        .weight(0.8);
    assert_eq!(layer.min_height, 10.0);
    assert_eq!(layer.max_height, 50.0);
    assert_eq!(layer.min_slope, 0.3);
    assert_eq!(layer.max_slope, 0.9);
    assert_eq!(layer.base_weight, 0.8);
}

#[test]
fn terrain_layer_compute_weight_in_range() {
    let layer = TerrainLayer::new("test")
        .height_range(0.0, 10.0)
        .slope_range(0.0, 0.5)
        .weight(1.0);
    assert_eq!(layer.compute_weight(5.0, 0.2), 1.0);
}

#[test]
fn terrain_layer_compute_weight_out_of_height() {
    let layer = TerrainLayer::new("test")
        .height_range(0.0, 10.0);
    assert_eq!(layer.compute_weight(-1.0, 0.0), 0.0);
    assert_eq!(layer.compute_weight(11.0, 0.0), 0.0);
}

#[test]
fn terrain_layer_compute_weight_out_of_slope() {
    let layer = TerrainLayer::new("test")
        .slope_range(0.0, 0.5);
    assert_eq!(layer.compute_weight(0.0, 0.6), 0.0);
}

#[test]
fn splat_map_new() {
    let map = SplatMap::new(4, 4, [0, 1, 2, 3]);
    assert_eq!(map.width, 4);
    assert_eq!(map.depth, 4);
    assert_eq!(map.weights.len(), 16);
    assert_eq!(map.weights[0], [0.0; 4]);
    assert_eq!(map.layer_indices, [0, 1, 2, 3]);
}

#[test]
fn splat_map_get_set() {
    let mut map = SplatMap::new(4, 4, [0, 1, 2, 3]);
    map.set(1, 1, [0.25, 0.25, 0.25, 0.25]);
    assert_eq!(map.get(1, 1), [0.25, 0.25, 0.25, 0.25]);
    assert_eq!(map.get(5, 5), [0.0; 4]); // out of bounds
}

#[test]
fn splat_map_normalize() {
    let mut map = SplatMap::new(2, 2, [0, 1, 2, 3]);
    map.set(0, 0, [1.0, 1.0, 1.0, 1.0]);
    map.set(1, 1, [2.0, 0.0, 0.0, 0.0]);
    map.normalize();
    assert_eq!(map.get(0, 0), [0.25, 0.25, 0.25, 0.25]);
    assert_eq!(map.get(1, 1), [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn splat_stack_new() {
    let layers = vec![
        TerrainLayer::new("grass"),
        TerrainLayer::new("dirt"),
        TerrainLayer::new("rock"),
    ];
    let stack = SplatStack::new(layers);
    assert_eq!(stack.maps.len(), 1); // 3 layers fit in one RGBA map
    assert_eq!(stack.maps[0].layer_indices, [0, 1, 2, 3]);
}

#[test]
fn splat_stack_new_multiple_maps() {
    let layers = vec![
        TerrainLayer::new("a"),
        TerrainLayer::new("b"),
        TerrainLayer::new("c"),
        TerrainLayer::new("d"),
        TerrainLayer::new("e"),
    ];
    let stack = SplatStack::new(layers);
    assert_eq!(stack.maps.len(), 2); // 5 layers need 2 maps
}

#[test]
fn splat_stack_resize() {
    let layers = vec![TerrainLayer::new("grass")];
    let mut stack = SplatStack::new(layers);
    stack.resize(8, 8);
    assert_eq!(stack.maps[0].width, 8);
    assert_eq!(stack.maps[0].depth, 8);
    assert_eq!(stack.maps[0].weights.len(), 64);
}

#[test]
fn splat_stack_generate() {
    let layers = vec![
        TerrainLayer::new("grass").height_range(0.0, 5.0),
        TerrainLayer::new("rock").height_range(5.0, 10.0),
    ];
    let mut stack = SplatStack::new(layers);
    let heights = vec![2.0f32; 16]; // 4x4 flat at height 2
    let slopes = vec![0.0f32; 16];
    stack.generate(&heights, &slopes, 4, 4);
    // Grass layer should have weight, rock should have none
    let w = stack.maps[0].get(0, 0);
    assert!(w[0] > 0.0); // grass
    assert_eq!(w[1], 0.0); // rock
}
