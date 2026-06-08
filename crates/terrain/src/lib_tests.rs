//! Tests for terrain heightmap and mesh generation.

use crate::{Heightmap, build_terrain_mesh, build_collision_mesh};

#[test]
fn heightmap_flat() {
    let hm = Heightmap::flat(4, 4, 5.0);
    assert_eq!(hm.width, 4);
    assert_eq!(hm.depth, 4);
    assert_eq!(hm.heights.len(), 16);
    for h in &hm.heights {
        assert_eq!(*h, 5.0);
    }
}

#[test]
fn heightmap_from_fn() {
    let hm = Heightmap::from_fn(3, 3, |x, z| x + z);
    assert_eq!(hm.get(0, 0), 0.0);
    assert_eq!(hm.get(1, 0), 1.0);
    assert_eq!(hm.get(0, 1), 1.0);
    assert_eq!(hm.get(2, 2), 4.0);
}

#[test]
fn heightmap_get_out_of_bounds() {
    let hm = Heightmap::flat(2, 2, 1.0);
    assert_eq!(hm.get(5, 5), 0.0); // out of bounds returns 0
}

#[test]
fn heightmap_set_and_get() {
    let mut hm = Heightmap::flat(2, 2, 0.0);
    hm.set(0, 1, 7.5);
    assert_eq!(hm.get(0, 1), 7.5);
}

#[test]
fn heightmap_set_out_of_bounds_is_noop() {
    let mut hm = Heightmap::flat(2, 2, 0.0);
    hm.set(10, 10, 99.0);
    assert_eq!(hm.heights.iter().sum::<f32>(), 0.0);
}

#[test]
fn build_terrain_mesh_produces_vertices_and_indices() {
    let hm = Heightmap::flat(3, 3, 1.0);
    let (verts, indices) = build_terrain_mesh(&hm, 1.0);
    // 3x3 grid = 9 vertices
    assert_eq!(verts.len(), 9);
    // 2x2 quads = 4 quads * 6 indices = 24 indices
    assert_eq!(indices.len(), 24);
}

#[test]
fn build_terrain_mesh_vertex_positions() {
    let hm = Heightmap::flat(2, 2, 3.0);
    let (verts, _) = build_terrain_mesh(&hm, 2.0);
    // vertices should be spaced by world_scale
    let p0 = verts[0].position;
    let p1 = verts[1].position;
    assert!((p1[0] - p0[0] - 2.0).abs() < 1e-4, "x spacing should be world_scale");
}

#[test]
fn build_collision_mesh_produces_triangles() {
    let hm = Heightmap::flat(3, 3, 1.0);
    let (verts, tris) = build_collision_mesh(&hm, 1.0);
    assert_eq!(verts.len(), 9);
    // 2x2 quads = 4 quads * 2 triangles = 8 triangles
    assert_eq!(tris.len(), 8);
    // Each triangle is 3 u32 indices
    for tri in &tris {
        assert!(tri[0] < 9);
        assert!(tri[1] < 9);
        assert!(tri[2] < 9);
    }
}
