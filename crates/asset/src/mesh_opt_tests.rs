//! Tests for mesh optimization functions.

use crate::mesh::{MeshAsset, Vertex};
use crate::mesh_opt::*;

fn triangle_mesh() -> MeshAsset {
    MeshAsset::new(
        vec![
            Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            Vertex::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            Vertex::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ],
        vec![0, 1, 2],
    )
}

fn quad_mesh() -> MeshAsset {
    MeshAsset::new(
        vec![
            Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            Vertex::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            Vertex::new([1.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            Vertex::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ],
        vec![0, 1, 2, 0, 2, 3],
    )
}

#[test]
fn optimize_vertex_cache_preserves_count() {
    let mesh = quad_mesh();
    let optimized = optimize_vertex_cache(&mesh);
    assert_eq!(optimized.vertices.len(), mesh.vertices.len());
    assert_eq!(optimized.indices.len(), mesh.indices.len());
}

#[test]
fn optimize_overdraw_preserves_count() {
    let mesh = quad_mesh();
    let cache = optimize_vertex_cache(&mesh);
    let optimized = optimize_overdraw(&cache, 1.05);
    assert_eq!(optimized.vertices.len(), mesh.vertices.len());
    assert_eq!(optimized.indices.len(), mesh.indices.len());
}

#[test]
fn optimize_vertex_fetch_preserves_count() {
    let mesh = quad_mesh();
    let optimized = optimize_vertex_fetch(&mesh);
    assert_eq!(optimized.indices.len(), mesh.indices.len());
}

#[test]
fn optimize_full_preserves_count() {
    let mesh = quad_mesh();
    let optimized = optimize_full(&mesh, 1.05);
    assert_eq!(optimized.indices.len(), mesh.indices.len());
}

#[test]
fn stripify_roundtrip() {
    let mesh = triangle_mesh();
    let strip = stripify(&mesh).unwrap();
    assert!(!strip.is_empty());
    let back = unstripify(&strip, u16::MAX).unwrap();
    assert_eq!(back.len(), mesh.indices.len());
}

#[test]
fn analyze_vertex_cache_returns_stats() {
    let mesh = quad_mesh();
    let stats = analyze_vertex_cache(&mesh, 16);
    assert_eq!(stats.vertices_total, mesh.vertices.len());
    assert!(stats.acmr > 0.0);
}

#[test]
fn analyze_overdraw_returns_stats() {
    let mesh = quad_mesh();
    let stats = analyze_overdraw(&mesh);
    assert!(stats.overdraw >= 1.0);
}

#[test]
fn build_meshlets_returns_clusters() {
    let mesh = quad_mesh();
    let meshlets = build_meshlets(&mesh, 64, 124, 0.0);
    assert!(!meshlets.is_empty());
    for m in &meshlets {
        assert!(!m.vertices.is_empty());
        assert!(!m.triangles.is_empty());
    }
}
