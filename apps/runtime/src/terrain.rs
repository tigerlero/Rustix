//! Terrain system for Rustix.
//!
//! Provides a heightmap-based terrain mesh with brush sculpting,
//! texture splat painting, and foliage scatter support.

use rustix_core::math::Vec3;
use rustix_render::mesh::{Mesh, Vertex};
use rustix_render::Renderer;

/// Terrain component stores editable heightmap and splat data.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Terrain {
    /// World-space size in meters (square).
    pub size: f32,
    /// Grid resolution (vertices per side).
    pub resolution: u32,
    /// Height values per vertex, row-major [y * resolution + x].
    pub heightmap: Vec<f32>,
    /// 4-channel splat blend weights per vertex.
    pub splat: Vec<[f32; 4]>,
    /// Name of the mesh in AppState::meshes.
    pub mesh_name: String,
}

impl Terrain {
    /// Create a flat terrain of given size and resolution.
    pub fn new(size: f32, resolution: u32) -> Self {
        let count = (resolution * resolution) as usize;
        Self {
            size,
            resolution,
            heightmap: vec![0.0; count],
            splat: vec![[1.0, 0.0, 0.0, 0.0]; count],
            mesh_name: "Terrain".to_string(),
        }
    }

    /// Get height at grid coordinate.
    pub fn height(&self, x: u32, z: u32) -> f32 {
        let ix = (z.min(self.resolution - 1) * self.resolution + x.min(self.resolution - 1)) as usize;
        self.heightmap.get(ix).copied().unwrap_or(0.0)
    }

    /// Set height at grid coordinate.
    pub fn set_height(&mut self, x: u32, z: u32, h: f32) {
        let ix = (z.min(self.resolution - 1) * self.resolution + x.min(self.resolution - 1)) as usize;
        if let Some(v) = self.heightmap.get_mut(ix) {
            *v = h;
        }
    }

    /// World-space position of a grid vertex.
    pub fn world_pos(&self, x: u32, z: u32) -> Vec3 {
        let half = self.size * 0.5;
        let step = self.size / (self.resolution - 1) as f32;
        Vec3::new(
            x as f32 * step - half,
            self.height(x, z),
            z as f32 * step - half,
        )
    }

    /// Compute normal at grid coordinate using central differences.
    pub fn normal_at(&self, x: u32, z: u32) -> [f32; 3] {
        let step = self.size / (self.resolution - 1) as f32;
        let left = self.height(x.saturating_sub(1), z);
        let right = self.height((x + 1).min(self.resolution - 1), z);
        let up = self.height(x, z.saturating_sub(1));
        let down = self.height(x, (z + 1).min(self.resolution - 1));
        let nx = left - right;
        let nz = up - down;
        let n = Vec3::new(nx, 2.0 * step, nz).normalize();
        [n.x, n.y, n.z]
    }

    /// Regenerate mesh from current heightmap.
    pub fn regenerate_mesh(&self, renderer: &Renderer) -> Result<Mesh, rustix_render::RenderError> {
        let mut verts = Vec::with_capacity((self.resolution * self.resolution) as usize);
        for z in 0..self.resolution {
            for x in 0..self.resolution {
                let pos = self.world_pos(x, z);
                let normal = self.normal_at(x, z);
                verts.push(Vertex { position: [pos.x, pos.y, pos.z], normal });
            }
        }
        let indices = quad_indices(self.resolution, self.resolution);
        let vert_bytes = bytemuck::cast_slice(&verts);
        Mesh::new(
            renderer,
            &self.mesh_name,
            vert_bytes,
            verts.len() as u32,
            Some((&indices, indices.len() as u32)),
        )
    }

    /// Apply a brush stroke at world-space position.
    pub fn brush(&mut self, world_pos: Vec3, radius: f32, strength: f32, mode: BrushMode) {
        let half = self.size * 0.5;
        let step = self.size / (self.resolution - 1) as f32;

        // Convert world position to grid coordinates
        let cx = ((world_pos.x + half) / step).round() as i32;
        let cz = ((world_pos.z + half) / step).round() as i32;
        let brush_px = (radius / step).ceil() as i32;

        for dz in -brush_px..=brush_px {
            for dx in -brush_px..=brush_px {
                let gx = cx + dx;
                let gz = cz + dz;
                if gx < 0 || gx >= self.resolution as i32 || gz < 0 || gz >= self.resolution as i32 {
                    continue;
                }
                let gx = gx as u32;
                let gz = gz as u32;

                let wp = self.world_pos(gx, gz);
                let dist = ((wp.x - world_pos.x).powi(2) + (wp.z - world_pos.z).powi(2)).sqrt();
                if dist > radius {
                    continue;
                }

                let falloff = (1.0 - dist / radius).max(0.0);
                let factor = falloff * strength;
                let ix = (gz * self.resolution + gx) as usize;

                match mode {
                    BrushMode::Raise => {
                        self.heightmap[ix] += factor;
                    }
                    BrushMode::Lower => {
                        self.heightmap[ix] -= factor;
                    }
                    BrushMode::Smooth => {
                        let h = self.heightmap[ix];
                        let mut sum = h;
                        let mut count = 1u32;
                        for dz2 in -1..=1i32 {
                            for dx2 in -1..=1i32 {
                                let nx = (gx as i32 + dx2).clamp(0, self.resolution as i32 - 1) as u32;
                                let nz = (gz as i32 + dz2).clamp(0, self.resolution as i32 - 1) as u32;
                                sum += self.height(nx, nz);
                                count += 1;
                            }
                        }
                        let avg = sum / count as f32;
                        self.heightmap[ix] = h + (avg - h) * factor;
                    }
                    BrushMode::Flatten => {
                        let target = world_pos.y;
                        let h = self.heightmap[ix];
                        self.heightmap[ix] = h + (target - h) * factor;
                    }
                    BrushMode::Splat(channel) => {
                        let ch = channel as usize;
                        self.splat[ix][ch] = (self.splat[ix][ch] + factor).min(1.0);
                        // Normalize other channels down
                        let sum: f32 = self.splat[ix].iter().sum();
                        if sum > 1.0 {
                            for c in 0..4 {
                                if c != ch {
                                    self.splat[ix][c] = (self.splat[ix][c] / sum).max(0.0);
                                }
                            }
                            self.splat[ix][ch] = (self.splat[ix][ch] / sum).max(0.0);
                        }
                    }
                }
            }
        }
    }

    /// Sample height at arbitrary world-space xz position using bilinear interpolation.
    pub fn sample_height(&self, x: f32, z: f32) -> f32 {
        let half = self.size * 0.5;
        let step = self.size / (self.resolution - 1) as f32;
        let fx = ((x + half) / step).clamp(0.0, (self.resolution - 1) as f32);
        let fz = ((z + half) / step).clamp(0.0, (self.resolution - 1) as f32);
        let ix = fx as u32;
        let iz = fz as u32;
        let tx = fx - ix as f32;
        let tz = fz - iz as f32;

        let h00 = self.height(ix, iz);
        let h10 = self.height((ix + 1).min(self.resolution - 1), iz);
        let h01 = self.height(ix, (iz + 1).min(self.resolution - 1));
        let h11 = self.height((ix + 1).min(self.resolution - 1), (iz + 1).min(self.resolution - 1));

        let h0 = h00 + (h10 - h00) * tx;
        let h1 = h01 + (h11 - h01) * tx;
        h0 + (h1 - h0) * tz
    }
}

/// Brush operation modes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrushMode {
    Raise,
    Lower,
    Smooth,
    Flatten,
    /// Paint texture channel 0..3.
    Splat(u8),
}

impl BrushMode {
    pub fn label(self) -> &'static str {
        match self {
            BrushMode::Raise => "Raise",
            BrushMode::Lower => "Lower",
            BrushMode::Smooth => "Smooth",
            BrushMode::Flatten => "Flatten",
            BrushMode::Splat(0) => "Paint Red",
            BrushMode::Splat(1) => "Paint Green",
            BrushMode::Splat(2) => "Paint Blue",
            BrushMode::Splat(3) => "Paint Alpha",
            BrushMode::Splat(_) => "Paint",
        }
    }
}

/// Terrain editor state.
#[derive(Debug, Clone)]
pub struct TerrainEditor {
    pub show: bool,
    pub brush_mode: BrushMode,
    pub brush_radius: f32,
    pub brush_strength: f32,
    pub splat_channel: u8,
    pub foliage_density: u32,
    pub foliage_scale: f32,
    pub regen_needed: bool,
}

impl Default for TerrainEditor {
    fn default() -> Self {
        Self {
            show: false,
            brush_mode: BrushMode::Raise,
            brush_radius: 5.0,
            brush_strength: 0.5,
            splat_channel: 0,
            foliage_density: 10,
            foliage_scale: 1.0,
            regen_needed: false,
        }
    }
}

/// Scatter foliage instances on the terrain surface.
/// Returns a Vec of (position, scale) for each instance.
pub fn scatter_foliage(terrain: &Terrain, density: u32, scale: f32, seed: u64) -> Vec<(Vec3, f32)> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut result = Vec::new();
    let half = terrain.size * 0.5;
    let mut counter = 0u64;
    let mut rng = || {
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        counter.hash(&mut hasher);
        counter += 1;
        let hash = hasher.finish();
        (hash as f32 / u64::MAX as f32)
    };

    for _ in 0..density {
        let rx = rng() * terrain.size - half;
        let rz = rng() * terrain.size - half;
        let ry = terrain.sample_height(rx, rz);
        let pos = Vec3::new(rx, ry, rz);
        let s = scale * (0.8 + rng() * 0.4);
        result.push((pos, s));
    }
    result
}

fn quad_indices(r: u32, s: u32) -> Vec<u16> {
    let mut idx = Vec::new();
    for i in 0..r - 1 {
        for j in 0..s - 1 {
            let a = i * s + j;
            let b = a + 1;
            let c = (i + 1) * s + j;
            let d = c + 1;
            idx.extend_from_slice(&[a as u16, b as u16, c as u16, c as u16, b as u16, d as u16]);
        }
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_flat() {
        let t = Terrain::new(100.0, 33);
        assert_eq!(t.height(0, 0), 0.0);
        assert_eq!(t.height(16, 16), 0.0);
        assert_eq!(t.resolution, 33);
    }

    #[test]
    fn terrain_brush_raise() {
        let mut t = Terrain::new(100.0, 33);
        t.brush(Vec3::new(0.0, 0.0, 0.0), 10.0, 1.0, BrushMode::Raise);
        assert!(t.height(16, 16) > 0.0);
    }

    #[test]
    fn terrain_sample_bilinear() {
        let mut t = Terrain::new(100.0, 33);
        t.set_height(16, 16, 5.0);
        let h = t.sample_height(0.0, 0.0);
        assert!(h > 0.0);
    }
}
