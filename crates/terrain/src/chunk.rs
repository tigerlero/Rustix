//! Chunked terrain with LOD and seamless stitching.
//!
//! A `TerrainChunk` represents a square patch of terrain with a
//! configurable mesh resolution. Neighboring chunks can have mismatched
//! LOD; skirt vertices hide T-junctions along chunk boundaries.

use crate::{Heightmap, TerrainVertex};

/// A single terrain chunk with mesh LOD.
#[derive(Debug, Clone)]
pub struct TerrainChunk {
    pub origin_x: f32,
    pub origin_z: f32,
    pub size: f32,
    pub resolution: usize,
    pub lod_level: u32,
    pub vertices: Vec<TerrainVertex>,
    pub indices: Vec<u16>,
}

impl TerrainChunk {
    /// Build a chunk from a subsection of a heightmap.
    ///
    /// `sample_step` controls the LOD: 1 = full resolution, 2 = half,
    /// 4 = quarter, etc.
    pub fn from_heightmap(
        heightmap: &Heightmap,
        chunk_x: usize,
        chunk_z: usize,
        chunk_size: usize,
        sample_step: usize,
        world_scale: f32,
    ) -> Self {
        let step = sample_step.max(1);
        let res = (chunk_size / step) + 1;
        let mut verts = Vec::with_capacity(res * res);
        let mut indices = Vec::with_capacity((res - 1) * (res - 1) * 6);

        for lz in 0..res {
            for lx in 0..res {
                let hx = (chunk_x + lx * step).min(heightmap.width - 1);
                let hz = (chunk_z + lz * step).min(heightmap.depth - 1);
                let h = heightmap.get(hx, hz);
                verts.push(TerrainVertex {
                    position: [
                        (chunk_x as f32 + lx as f32 * step as f32) * world_scale,
                        h,
                        (chunk_z as f32 + lz as f32 * step as f32) * world_scale,
                    ],
                    normal: [0.0, 1.0, 0.0],
                });
            }
        }

        // Generate indices
        for z in 0..res - 1 {
            for x in 0..res - 1 {
                let a = (z * res + x) as u16;
                let b = a + 1;
                let c = ((z + 1) * res + x) as u16;
                let d = c + 1;
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }

        // Compute normals
        for z in 1..res - 1 {
            for x in 1..res - 1 {
                let idx = z * res + x;
                let left = verts[idx - 1].position;
                let right = verts[idx + 1].position;
                let up = verts[idx - res].position;
                let down = verts[idx + res].position;
                let dx = crate::Vec3::new(right[0] - left[0], right[1] - left[1], right[2] - left[2]);
                let dz = crate::Vec3::new(down[0] - up[0], down[1] - up[1], down[2] - up[2]);
                let n = dz.cross(dx).normalize();
                verts[idx].normal = [n.x, n.y, n.z];
            }
        }

        let origin_x = chunk_x as f32 * world_scale;
        let origin_z = chunk_z as f32 * world_scale;
        let size = chunk_size as f32 * world_scale;

        Self {
            origin_x,
            origin_z,
            size,
            resolution: res,
            lod_level: step.trailing_zeros(),
            vertices: verts,
            indices,
        }
    }
}

/// A quadtree-based terrain manager that splits the world into chunks
/// with distance-based LOD.
#[derive(Debug, Clone)]
pub struct ChunkedTerrain {
    pub chunks: Vec<TerrainChunk>,
    pub chunk_size: usize,
    pub world_scale: f32,
    pub lod_distances: Vec<f32>,
}

impl ChunkedTerrain {
    pub fn new(chunk_size: usize, world_scale: f32, lod_distances: Vec<f32>) -> Self {
        Self {
            chunks: Vec::new(),
            chunk_size,
            world_scale,
            lod_distances,
        }
    }

    /// Rebuild all chunks from a heightmap using distance-based LOD.
    pub fn rebuild(&mut self, heightmap: &Heightmap, view_x: f32, view_z: f32) {
        self.chunks.clear();
        let width_chunks = (heightmap.width + self.chunk_size - 1) / self.chunk_size;
        let depth_chunks = (heightmap.depth + self.chunk_size - 1) / self.chunk_size;

        for cz in 0..depth_chunks {
            for cx in 0..width_chunks {
                let chunk_x = cx * self.chunk_size;
                let chunk_z = cz * self.chunk_size;
                let chunk_center_x = (chunk_x as f32 + self.chunk_size as f32 * 0.5) * self.world_scale;
                let chunk_center_z = (chunk_z as f32 + self.chunk_size as f32 * 0.5) * self.world_scale;
                let dist = ((chunk_center_x - view_x).powi(2) + (chunk_center_z - view_z).powi(2)).sqrt();

                let sample_step = self
                    .lod_distances
                    .iter()
                    .enumerate()
                    .find(|&(_, &d)| dist < d)
                    .map(|(i, _)| 1usize << i)
                    .unwrap_or(1 << (self.lod_distances.len().saturating_sub(1)));

                self.chunks.push(TerrainChunk::from_heightmap(
                    heightmap,
                    chunk_x,
                    chunk_z,
                    self.chunk_size,
                    sample_step,
                    self.world_scale,
                ));
            }
        }
    }

    /// Load / unload chunks based on a maximum streaming radius.
    pub fn stream_chunks(&mut self, heightmap: &Heightmap, view_x: f32, view_z: f32, max_radius: f32) {
        let width_chunks = (heightmap.width + self.chunk_size - 1) / self.chunk_size;
        let depth_chunks = (heightmap.depth + self.chunk_size - 1) / self.chunk_size;

        // Retain only chunks still within radius
        self.chunks.retain(|c| {
            let dx = (c.origin_x + c.size * 0.5) - view_x;
            let dz = (c.origin_z + c.size * 0.5) - view_z;
            (dx * dx + dz * dz).sqrt() <= max_radius
        });

        // Build a set of active chunk origins for fast lookup
        let active: std::collections::HashSet<(usize, usize)> = self
            .chunks
            .iter()
            .map(|c| ((c.origin_x / self.world_scale) as usize, (c.origin_z / self.world_scale) as usize))
            .collect();

        for cz in 0..depth_chunks {
            for cx in 0..width_chunks {
                let chunk_x = cx * self.chunk_size;
                let chunk_z = cz * self.chunk_size;
                if active.contains(&(chunk_x, chunk_z)) {
                    continue;
                }
                let chunk_center_x = (chunk_x as f32 + self.chunk_size as f32 * 0.5) * self.world_scale;
                let chunk_center_z = (chunk_z as f32 + self.chunk_size as f32 * 0.5) * self.world_scale;
                let dx = chunk_center_x - view_x;
                let dz = chunk_center_z - view_z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist > max_radius {
                    continue;
                }

                let sample_step = self
                    .lod_distances
                    .iter()
                    .enumerate()
                    .find(|&(_, &d)| dist < d)
                    .map(|(i, _)| 1usize << i)
                    .unwrap_or(1 << self.lod_distances.len().saturating_sub(1));

                self.chunks.push(TerrainChunk::from_heightmap(
                    heightmap,
                    chunk_x,
                    chunk_z,
                    self.chunk_size,
                    sample_step,
                    self.world_scale,
                ));
            }
        }
    }
}

/// Generate skirt vertices along the edges of a chunk to hide cracks
/// between chunks of different LOD.
///
/// Returns `( skirt_vertices, skirt_indices )` that can be rendered
/// as a vertical wall around the chunk perimeter.
pub fn build_chunk_skirt(
    chunk: &TerrainChunk,
    neighbor_edge_heights: [Option<&[f32]>; 4],
    skirt_depth: f32,
) -> (Vec<TerrainVertex>, Vec<u16>) {
    let res = chunk.resolution;
    let mut skirt_verts = Vec::new();
    let mut skirt_indices = Vec::new();

    // Helper to add a skirt strip between two edges
    let mut add_strip = |edge0: Vec<usize>, _edge1: Vec<usize>, edge1_heights: Option<&[f32]>| {
        let base = skirt_verts.len() as u16;
        for (i, &idx0) in edge0.iter().enumerate() {
            let v0 = chunk.vertices[idx0];
            skirt_verts.push(v0);

            let v1_pos = if let Some(heights) = edge1_heights {
                let h = *heights.get(i).unwrap_or(&v0.position[1]);
                [v0.position[0], h, v0.position[2]]
            } else {
                [v0.position[0], v0.position[1] - skirt_depth, v0.position[2]]
            };
            skirt_verts.push(TerrainVertex {
                position: v1_pos,
                normal: [0.0, 0.0, 0.0],
            });
        }

        let len = edge0.len() as u16;
        for i in 0..len - 1 {
            let a = base + i * 2;
            let b = a + 1;
            let c = a + 2;
            let d = a + 3;
            skirt_indices.extend_from_slice(&[a, b, c, c, b, d]);
        }
    };

    // Top edge (z = 0)
    let top: Vec<usize> = (0..res).map(|x| x).collect();
    let top_next: Vec<usize> = (0..res).map(|x| x + res).collect();
    add_strip(top, top_next, neighbor_edge_heights[0]);

    // Bottom edge (z = res-1)
    let bottom: Vec<usize> = (0..res).map(|x| (res - 1) * res + x).collect();
    let bottom_next: Vec<usize> = (0..res).map(|x| (res - 2) * res + x).collect();
    add_strip(bottom, bottom_next, neighbor_edge_heights[1]);

    // Left edge (x = 0)
    let left: Vec<usize> = (0..res).map(|z| z * res).collect();
    let left_next: Vec<usize> = (0..res).map(|z| z * res + 1).collect();
    add_strip(left, left_next, neighbor_edge_heights[2]);

    // Right edge (x = res-1)
    let right: Vec<usize> = (0..res).map(|z| z * res + (res - 1)).collect();
    let right_next: Vec<usize> = (0..res).map(|z| z * res + (res - 2)).collect();
    add_strip(right, right_next, neighbor_edge_heights[3]);

    (skirt_verts, skirt_indices)
}
