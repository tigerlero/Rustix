//! Mesh optimization: vertex cache reordering, overdraw reduction, and
//! stripification.
//!
//! `MeshOptimizer` wraps the `meshopt` crate (meshoptimizer by Arseny
//! Kapoulkine) to improve GPU vertex cache hit rates, reduce overdraw,
//! and convert triangle lists to triangle strips.

use crate::mesh::{MeshAsset, Vertex};

impl meshopt::DecodePosition for Vertex {
    fn decode_position(&self) -> [f32; 3] {
        self.position
    }
}

/// Reorder mesh indices for optimal GPU vertex cache hit rate.
///
/// Uses the Forsyth algorithm (meshoptimizer's `optimizeVertexCache`).
pub fn optimize_vertex_cache(mesh: &MeshAsset) -> MeshAsset {
    let indices_u32: Vec<u32> = mesh.indices.iter().map(|&i| i as u32).collect();
    let vertex_count = mesh.vertices.len();

    let optimized = meshopt::optimize_vertex_cache(&indices_u32, vertex_count);

    MeshAsset {
        vertices: mesh.vertices.clone(),
        indices: optimized.into_iter().map(|i| i as u16).collect(),
        aabb: mesh.aabb,
    }
}

/// Reorder triangles to reduce overdraw from a given camera direction.
///
/// `threshold` controls the trade-off between vertex cache and overdraw
/// (1.05 = balanced, 1.0 = pure overdraw, 1.2 = vertex cache heavy).
///
/// The input mesh must already be vertex-cache optimized (e.g. from
/// `optimize_vertex_cache`).
pub fn optimize_overdraw(mesh: &MeshAsset, threshold: f32) -> MeshAsset {
    let mut indices_u32: Vec<u32> = mesh.indices.iter().map(|&i| i as u32).collect();

    meshopt::optimize_overdraw_in_place_decoder(&mut indices_u32, &mesh.vertices, threshold);

    MeshAsset {
        vertices: mesh.vertices.clone(),
        indices: indices_u32.into_iter().map(|i| i as u16).collect(),
        aabb: mesh.aabb,
    }
}

/// Reorder vertices and indices for optimal vertex fetch locality.
///
/// Removes duplicate vertices, reorders the vertex buffer for cache
/// locality, and remaps indices accordingly. Often used as a final step
/// after cache and overdraw optimization.
pub fn optimize_vertex_fetch(mesh: &MeshAsset) -> MeshAsset {
    let mut indices_u32: Vec<u32> = mesh.indices.iter().map(|&i| i as u32).collect();

    let new_vertices = meshopt::optimize_vertex_fetch(&mut indices_u32, &mesh.vertices);

    MeshAsset {
        vertices: new_vertices,
        indices: indices_u32.into_iter().map(|i| i as u16).collect(),
        aabb: mesh.aabb,
    }
}

/// Full optimization pipeline: vertex cache → overdraw → vertex fetch.
///
/// Applies the three optimizations in the recommended order and returns
/// a fully optimized mesh with deduplicated, reordered vertices and
/// indices tuned for GPU performance.
pub fn optimize_full(mesh: &MeshAsset, overdraw_threshold: f32) -> MeshAsset {
    let cache = optimize_vertex_cache(mesh);
    let overdraw = optimize_overdraw(&cache, overdraw_threshold);
    optimize_vertex_fetch(&overdraw)
}

/// Convert a triangle list to a triangle strip.
///
/// Returns the strip indices. The caller can render with primitive
/// restart (`0xFFFF` for u16 indices) or split on restart values.
pub fn stripify(mesh: &MeshAsset) -> Result<Vec<u16>, String> {
    let indices_u32: Vec<u32> = mesh.indices.iter().map(|&i| i as u32).collect();
    let vertex_count = mesh.vertices.len();

    let strip = meshopt::stripify(&indices_u32, vertex_count, u32::MAX)
        .map_err(|e| format!("stripify failed: {e}"))?;

    Ok(strip.into_iter().map(|i| {
        if i == u32::MAX {
            u16::MAX // restart marker
        } else {
            i as u16
        }
    }).collect())
}

/// Convert a triangle strip back to a triangle list.
pub fn unstripify(strip: &[u16], restart_index: u16) -> Result<Vec<u16>, String> {
    let strip_u32: Vec<u32> = strip
        .iter()
        .map(|&i| if i == restart_index { u32::MAX } else { i as u32 })
        .collect();

    meshopt::unstripify(&strip_u32, u32::MAX)
        .map_err(|e| format!("unstripify failed: {e}"))
        .map(|v| v.into_iter().map(|i| i as u16).collect())
}

/// Compute meshlet clusters for GPU mesh shading.
///
/// `max_vertices` — max unique vertices per meshlet (≤ 256, e.g. 64).
/// `max_triangles` — max triangles per meshlet (≤ 512, e.g. 126).
/// `cone_weight` — weight for cone culling heuristic (0 = ignore,
/// 1 = strong cone culling).
///
/// Returns a list of meshlets, each containing local vertex indices
/// and packed triangle indices.
pub fn build_meshlets(
    mesh: &MeshAsset,
    max_vertices: usize,
    max_triangles: usize,
    cone_weight: f32,
) -> Vec<Meshlet> {
    let indices_u32: Vec<u32> = mesh.indices.iter().map(|&i| i as u32).collect();
    let vertex_bytes = meshopt::typed_to_bytes(&mesh.vertices);
    let adapter = meshopt::VertexDataAdapter::new(vertex_bytes, std::mem::size_of::<Vertex>(), 0)
        .expect("vertex data adapter");

    let meshlets = meshopt::build_meshlets(
        &indices_u32,
        &adapter,
        max_vertices,
        max_triangles,
        cone_weight,
    );

    meshlets
        .iter()
        .map(|m| Meshlet {
            vertices: m.vertices.to_vec(),
            triangles: m.triangles.to_vec(),
        })
        .collect()
}

/// A single meshlet cluster for GPU mesh shading.
#[derive(Debug, Clone)]
pub struct Meshlet {
    /// Local vertex indices (referencing the original vertex buffer).
    pub vertices: Vec<u32>,
    /// Packed triangle indices (3 indices per triangle, 8-bit each).
    pub triangles: Vec<u8>,
}

/// Vertex cache statistics for a mesh.
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub vertices_transformed: u32,
    pub warps_executed: u32,
    pub vertices_total: usize,
    /// Average ACMR (Average Cache Miss Ratio) — lower is better.
    pub acmr: f32,
    /// Average ATVR (Average Transform to Vertex Ratio) — closer to 1.0 is better.
    pub atvr: f32,
}

/// Analyze vertex cache efficiency of a mesh.
pub fn analyze_vertex_cache(mesh: &MeshAsset, cache_size: u32) -> CacheStats {
    let indices_u32: Vec<u32> = mesh.indices.iter().map(|&i| i as u32).collect();
    let vertex_count = mesh.vertices.len();

    let stats = meshopt::analyze_vertex_cache(&indices_u32, vertex_count, cache_size, 32, 128);

    CacheStats {
        vertices_transformed: stats.vertices_transformed,
        warps_executed: stats.warps_executed,
        vertices_total: vertex_count,
        acmr: stats.acmr,
        atvr: stats.atvr,
    }
}

/// Analyze overdraw of a mesh.
pub fn analyze_overdraw(mesh: &MeshAsset) -> OverdrawStats {
    let indices_u32: Vec<u32> = mesh.indices.iter().map(|&i| i as u32).collect();

    let stats = meshopt::analyze_overdraw_decoder(&indices_u32, &mesh.vertices);

    OverdrawStats {
        pixels_covered: stats.pixels_covered,
        pixels_shaded: stats.pixels_shaded,
        overdraw: stats.overdraw,
    }
}

/// Overdraw statistics for a mesh.
#[derive(Debug, Clone, Copy)]
pub struct OverdrawStats {
    pub pixels_covered: u32,
    pub pixels_shaded: u32,
    /// Shaded pixels / covered pixels — closer to 1.0 is better.
    pub overdraw: f32,
}
