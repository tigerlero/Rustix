//! Tests for terrain chunk, material, foliage, and import modules.

use rustix_core::math::{Vec3, Quat};
use crate::*;
use crate::chunk::*;
use crate::material::*;
use crate::foliage::*;

// ---------- chunk.rs ----------

#[test]
fn terrain_chunk_from_flat_heightmap() {
    let hm = Heightmap::flat(16, 16, 5.0);
    let chunk = TerrainChunk::from_heightmap(&hm, 0, 0, 8, 1, 1.0);
    assert_eq!(chunk.resolution, 9); // (8 / 1) + 1
    assert_eq!(chunk.size, 8.0);
    assert_eq!(chunk.origin_x, 0.0);
    assert_eq!(chunk.origin_z, 0.0);
    assert_eq!(chunk.lod_level, 0);
    assert_eq!(chunk.vertices.len(), 81);
    // indices: (9-1)*(9-1)*6 = 384
    assert_eq!(chunk.indices.len(), 384);
}

#[test]
fn terrain_chunk_from_heightmap_with_lod() {
    let hm = Heightmap::flat(16, 16, 3.0);
    let chunk = TerrainChunk::from_heightmap(&hm, 0, 0, 8, 2, 1.0);
    assert_eq!(chunk.resolution, 5); // (8 / 2) + 1
    assert_eq!(chunk.vertices.len(), 25);
    assert_eq!(chunk.lod_level, 1);
}

#[test]
fn terrain_chunk_normals_on_flat_terrain() {
    let hm = Heightmap::flat(16, 16, 5.0);
    let chunk = TerrainChunk::from_heightmap(&hm, 0, 0, 8, 1, 1.0);
    // For flat terrain, interior normals should point up
    let idx = chunk.resolution + 1; // interior point at (1, 1)
    let normal = chunk.vertices[idx].normal;
    assert!(normal[1] > 0.9); // mostly up
}

#[test]
fn terrain_chunk_positions_scaled() {
    let hm = Heightmap::flat(16, 16, 2.0);
    let chunk = TerrainChunk::from_heightmap(&hm, 0, 0, 4, 1, 2.0);
    // Last vertex should be at x = 3 * 2.0 = 6.0
    let last = chunk.vertices.last().unwrap();
    assert!((last.position[0] - 6.0).abs() < 1e-3);
}

#[test]
fn chunked_terrain_new_empty() {
    let ct = ChunkedTerrain::new(16, 1.0, vec![50.0, 150.0, 300.0]);
    assert!(ct.chunks.is_empty());
    assert_eq!(ct.chunk_size, 16);
    assert_eq!(ct.world_scale, 1.0);
}

#[test]
fn chunked_terrain_rebuild() {
    let hm = Heightmap::flat(32, 32, 0.0);
    let mut ct = ChunkedTerrain::new(16, 1.0, vec![100.0]);
    ct.rebuild(&hm, 0.0, 0.0);
    assert_eq!(ct.chunks.len(), 4); // 2x2 chunks for 32x32 with chunk_size 16
}

#[test]
fn chunked_terrain_stream_chunks() {
    let hm = Heightmap::flat(32, 32, 0.0);
    let mut ct = ChunkedTerrain::new(16, 1.0, vec![100.0]);
    ct.rebuild(&hm, 0.0, 0.0);
    assert_eq!(ct.chunks.len(), 4);

    // Stream with small radius should remove far chunks
    ct.stream_chunks(&hm, 0.0, 0.0, 5.0);
    // Only the chunk near (0,0) should remain
    assert!(ct.chunks.len() < 4);
}

#[test]
fn build_chunk_skirt() {
    let hm = Heightmap::flat(8, 8, 1.0);
    let chunk = TerrainChunk::from_heightmap(&hm, 0, 0, 8, 1, 1.0);
    let (skirt_verts, skirt_indices) = build_chunk_skirt(&chunk, [None, None, None, None], 2.0);
    assert!(!skirt_verts.is_empty());
    assert!(!skirt_indices.is_empty());
}

// ---------- material.rs ----------

#[test]
fn terrain_material_default() {
    let m = TerrainMaterial::default();
    assert_eq!(m.albedo, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(m.roughness, 0.8);
    assert_eq!(m.ao, 1.0);
    assert_eq!(m.metalness, 0.0);
    assert_eq!(m.normal_strength, 1.0);
}

#[test]
fn terrain_material_builder() {
    let m = TerrainMaterial::new()
        .albedo(0.5, 0.5, 0.5, 1.0)
        .roughness(0.3)
        .ao(0.9)
        .metalness(0.1)
        .normal_strength(2.0);
    assert_eq!(m.albedo, [0.5, 0.5, 0.5, 1.0]);
    assert!((m.roughness - 0.3).abs() < 1e-4);
    assert!((m.ao - 0.9).abs() < 1e-4);
    assert!((m.metalness - 0.1).abs() < 1e-4);
    assert_eq!(m.normal_strength, 2.0);
}

#[test]
fn terrain_material_clamping() {
    let m = TerrainMaterial::new().roughness(1.5).metalness(-0.5);
    assert_eq!(m.roughness, 1.0);
    assert_eq!(m.metalness, 0.0);
}

#[test]
fn terrain_material_palette_get() {
    let palette = TerrainMaterialPalette::new(vec![
        TerrainMaterial::new().roughness(0.2),
        TerrainMaterial::new().roughness(0.8),
    ]);
    assert!((palette.get(0).roughness - 0.2).abs() < 1e-4);
    assert!((palette.get(1).roughness - 0.8).abs() < 1e-4);
}

#[test]
fn terrain_material_palette_out_of_bounds_uses_default() {
    let palette = TerrainMaterialPalette::new(vec![]);
    let m = palette.get(0);
    assert_eq!(m.albedo, [1.0, 1.0, 1.0, 1.0]);
}

// ---------- foliage.rs ----------

#[test]
fn foliage_instance_to_matrix() {
    let instance = FoliageInstance {
        position: Vec3::new(1.0, 2.0, 3.0),
        scale: 2.0,
        rotation: Quat::IDENTITY,
        layer_index: 0,
    };
    let mat = instance.to_matrix();
    // Scale should be 2x
    let scale = mat.w_axis;
    assert!((scale.x - 1.0).abs() < 1e-3); // translation in last column
}

#[test]
fn foliage_layer_new_default() {
    let layer = FoliageLayer::new();
    assert_eq!(layer.min_height, f32::NEG_INFINITY);
    assert_eq!(layer.max_height, f32::INFINITY);
    assert_eq!(layer.max_slope, 0.7);
    assert_eq!(layer.min_scale, 0.8);
    assert_eq!(layer.max_scale, 1.2);
}

#[test]
fn foliage_layer_builder() {
    let layer = FoliageLayer::new()
        .height_range(10.0, 50.0)
        .max_slope(0.5)
        .scale_range(0.5, 1.5);
    assert_eq!(layer.min_height, 10.0);
    assert_eq!(layer.max_height, 50.0);
    assert_eq!(layer.max_slope, 0.5);
    assert_eq!(layer.min_scale, 0.5);
    assert_eq!(layer.max_scale, 1.5);
}

#[test]
fn scatter_foliage_generates_instances() {
    let hm = Heightmap::flat(4, 4, 0.0);
    let layer = FoliageLayer::new().height_range(-1.0, 1.0);
    let mut rng = || 0.5f32;
    let instances = scatter_foliage(&hm, 1.0, 1.0, &[layer], &mut rng);
    assert!(!instances.is_empty());
}

// ---------- import.rs ----------

#[test]
fn import_raw_success() {
    let bytes = vec![0u8, 128u8, 255u8, 64u8];
    let heights = crate::import::import_raw(&bytes, 2, 2).unwrap();
    assert_eq!(heights.len(), 4);
    assert!((heights[0] - 0.0).abs() < 1e-4);
    assert!((heights[1] - 128.0 / 255.0).abs() < 1e-4);
    assert!((heights[2] - 1.0).abs() < 1e-4);
    assert!((heights[3] - 64.0 / 255.0).abs() < 1e-4);
}

#[test]
fn import_raw_wrong_size() {
    let bytes = vec![0u8; 5];
    assert!(crate::import::import_raw(&bytes, 2, 2).is_err());
}

#[test]
fn import_r16_success() {
    let bytes = vec![0u8, 0u8, 0x80u8, 0x00u8, 0xFFu8, 0xFFu8, 0x40u8, 0x00u8];
    let heights = crate::import::import_r16(&bytes, 2, 2).unwrap();
    assert_eq!(heights.len(), 4);
    assert!((heights[0] - 0.0).abs() < 1e-4);
    assert!((heights[1] - 0x8000 as f32 / 65535.0).abs() < 1e-4);
    assert!((heights[2] - 1.0).abs() < 1e-4);
    assert!((heights[3] - 0x4000 as f32 / 65535.0).abs() < 1e-4);
}

#[test]
fn import_r16_wrong_size() {
    let bytes = vec![0u8; 6];
    assert!(crate::import::import_r16(&bytes, 2, 2).is_err());
}
