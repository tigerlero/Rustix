//! Tests for mesh asset types and binary format.

use rustix_core::math::{Vec3, Aabb};
use crate::mesh::{Vertex, MeshAsset, import_rxmesh, export_rxmesh};

#[test]
fn vertex_new() {
    let v = Vertex::new([1.0, 2.0, 3.0], [0.0, 1.0, 0.0]);
    assert_eq!(v.position, [1.0, 2.0, 3.0]);
    assert_eq!(v.normal, [0.0, 1.0, 0.0]);
}

#[test]
fn vertex_default() {
    let v = Vertex::default();
    assert_eq!(v.position, [0.0; 3]);
    assert_eq!(v.normal, [0.0; 3]);
}

#[test]
fn mesh_asset_new_computes_aabb() {
    let verts = vec![
        Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        Vertex::new([1.0, 2.0, 3.0], [0.0, 1.0, 0.0]),
    ];
    let mesh = MeshAsset::new(verts, vec![0, 1]);
    assert_eq!(mesh.vertex_count(), 2);
    assert_eq!(mesh.index_count(), 2);
    assert!(mesh.has_indices());
    assert_eq!(mesh.aabb.min, Vec3::new(0.0, 0.0, 0.0));
    assert_eq!(mesh.aabb.max, Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn mesh_asset_empty_aabb() {
    let mesh = MeshAsset::new(vec![], vec![]);
    assert_eq!(mesh.vertex_count(), 0);
    assert_eq!(mesh.index_count(), 0);
    assert!(!mesh.has_indices());
    assert_eq!(mesh.aabb, Aabb::new(Vec3::ZERO, Vec3::ZERO));
}

#[test]
fn mesh_asset_no_indices() {
    let mesh = MeshAsset::new(vec![Vertex::new([0.0; 3], [0.0; 3])], vec![]);
    assert!(!mesh.has_indices());
}

#[test]
fn rxmesh_roundtrip() {
    let original = MeshAsset::new(
        vec![
            Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            Vertex::new([0.0, 1.0, 0.0], [0.0, 1.0, 0.0]),
        ],
        vec![0, 1, 2],
    );
    let bytes = export_rxmesh(&original);
    let imported = import_rxmesh(&bytes).unwrap();
    assert_eq!(imported.vertices, original.vertices);
    assert_eq!(imported.indices, original.indices);
    assert_eq!(imported.aabb.min, original.aabb.min);
    assert_eq!(imported.aabb.max, original.aabb.max);
}

#[test]
fn rxmesh_invalid_magic() {
    let result = import_rxmesh(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxmesh_too_small() {
    let result = import_rxmesh(b"RXM1");
    assert!(result.is_err());
}

#[test]
fn rxmesh_empty_roundtrip() {
    let original = MeshAsset::new(vec![], vec![]);
    let bytes = export_rxmesh(&original);
    let imported = import_rxmesh(&bytes).unwrap();
    assert!(imported.vertices.is_empty());
    assert!(imported.indices.is_empty());
}
