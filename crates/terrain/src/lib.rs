use rustix_core::math::Vec3;

pub mod noise;
pub mod import;
pub mod chunk;
pub mod splat;
pub mod material;
pub mod foliage;
pub mod sculpt;
pub mod erosion;
pub mod water;

#[cfg(test)]
pub mod noise_tests;
#[cfg(test)]
pub mod lib_tests;
#[cfg(test)]
pub mod sculpt_tests;
#[cfg(test)]
pub mod water_tests;
#[cfg(test)]
pub mod chunk_tests;

/// 2D grid of height values.
#[derive(Debug, Clone)]
pub struct Heightmap {
    pub width: usize,
    pub depth: usize,
    pub heights: Vec<f32>,
}

impl Heightmap {
    pub fn flat(width: usize, depth: usize, height: f32) -> Self {
        Self { width, depth, heights: vec![height; width * depth] }
    }

    pub fn from_fn(width: usize, depth: usize, f: impl Fn(f32, f32) -> f32) -> Self {
        let mut heights = vec![0.0; width * depth];
        for z in 0..depth {
            for x in 0..width {
                heights[z * width + x] = f(x as f32, z as f32);
            }
        }
        Self { width, depth, heights }
    }

    pub fn get(&self, x: usize, z: usize) -> f32 {
        if x >= self.width || z >= self.depth { 0.0 } else { self.heights[z * self.width + x] }
    }

    pub fn set(&mut self, x: usize, z: usize, h: f32) {
        if x < self.width && z < self.depth {
            self.heights[z * self.width + x] = h;
        }
    }

    /// Import from a PNG grayscale image.
    pub fn from_png(bytes: &[u8]) -> Result<Self, String> {
        let (heights, width, height) = import::import_png(bytes)?;
        Ok(Self { width, depth: height, heights })
    }

    /// Import from an 8-bit raw binary file.
    pub fn from_raw(bytes: &[u8], width: usize, height: usize) -> Result<Self, String> {
        let heights = import::import_raw(bytes, width, height)?;
        Ok(Self { width, depth: height, heights })
    }

    /// Import from a 16-bit big-endian `.r16` file.
    pub fn from_r16(bytes: &[u8], width: usize, height: usize) -> Result<Self, String> {
        let heights = import::import_r16(bytes, width, height)?;
        Ok(Self { width, depth: height, heights })
    }
}

/// Vertex for terrain mesh (position + normal).
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TerrainVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// Generate a terrain mesh from a heightmap.
/// `scale` controls world-space size per grid cell.
pub fn build_terrain_mesh(heightmap: &Heightmap, scale: f32) -> (Vec<TerrainVertex>, Vec<u16>) {
    let w = heightmap.width;
    let d = heightmap.depth;
    let mut verts = Vec::with_capacity(w * d);
    let mut indices = Vec::with_capacity((w - 1) * (d - 1) * 6);

    for z in 0..d {
        for x in 0..w {
            let h = heightmap.get(x, z);
            verts.push(TerrainVertex {
                position: [x as f32 * scale, h, z as f32 * scale],
                normal: [0.0, 1.0, 0.0],
            });
        }
    }

    // Compute normals from face contributions
    for z in 1..d - 1 {
        for x in 1..w - 1 {
            let idx = z * w + x;
            let left = verts[idx - 1].position;
            let right = verts[idx + 1].position;
            let up = verts[idx - w].position;
            let down = verts[idx + w].position;
            let dx = Vec3::new(right[0] - left[0], right[1] - left[1], right[2] - left[2]);
            let dz = Vec3::new(down[0] - up[0], down[1] - up[1], down[2] - up[2]);
            let n = dz.cross(dx).normalize();
            verts[idx].normal = [n.x, n.y, n.z];
        }
    }

    for z in 0..d - 1 {
        for x in 0..w - 1 {
            let a = (z * w + x) as u16;
            let b = a + 1;
            let c = ((z + 1) * w + x) as u16;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    (verts, indices)
}

/// Generate a physics-friendly triangle soup from a heightmap.
/// Returns `(vertices, indices)` where each triangle is a separate
/// face suitable for trimesh collision.
pub fn build_collision_mesh(heightmap: &Heightmap, scale: f32) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let w = heightmap.width;
    let d = heightmap.depth;
    let mut verts = Vec::with_capacity(w * d);

    for z in 0..d {
        for x in 0..w {
            let h = heightmap.get(x, z);
            verts.push([x as f32 * scale, h, z as f32 * scale]);
        }
    }

    let mut tris = Vec::with_capacity((w - 1) * (d - 1) * 2);
    for z in 0..d - 1 {
        for x in 0..w - 1 {
            let a = (z * w + x) as u32;
            let b = a + 1;
            let c = ((z + 1) * w + x) as u32;
            let d = c + 1;
            tris.push([a, c, b]);
            tris.push([b, c, d]);
        }
    }

    (verts, tris)
}

/// Which noise algorithm to use for terrain generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseType {
    Value,
    Perlin,
}

/// Terrain generation parameters.
#[derive(Debug, Clone, Copy)]
pub struct TerrainParams {
    pub seed: u32,
    pub width: usize,
    pub depth: usize,
    pub scale: f32,
    pub height_scale: f32,
    pub octaves: u32,
    pub persistence: f32,
    pub lacunarity: f32,
    pub noise_type: NoiseType,
    /// Domain warp amplitude (0 = disabled).
    pub warp_amplitude: f32,
    /// Domain warp frequency.
    pub warp_frequency: f32,
}

impl Default for TerrainParams {
    fn default() -> Self {
        Self {
            seed: 42,
            width: 64,
            depth: 64,
            scale: 1.0,
            height_scale: 10.0,
            octaves: 4,
            persistence: 0.5,
            lacunarity: 2.0,
            noise_type: NoiseType::Value,
            warp_amplitude: 0.0,
            warp_frequency: 0.1,
        }
    }
}

/// Generate a heightmap from terrain parameters.
pub fn generate_heightmap(params: &TerrainParams) -> Heightmap {
    let perlin = noise::Perlin::new(params.seed);
    Heightmap::from_fn(params.width, params.depth, |x, z| {
        let sx = x / params.width as f32 * 4.0;
        let sz = z / params.depth as f32 * 4.0;
        let n = if params.warp_amplitude > 0.0 {
            match params.noise_type {
                NoiseType::Value => noise::domain_warp(
                    sx, sz,
                    params.warp_amplitude, params.warp_frequency,
                    |x, z| noise::fbm(x, z, params.seed, 2, params.persistence, params.lacunarity),
                    |x, z| noise::fbm(x, z, params.seed, params.octaves, params.persistence, params.lacunarity),
                ),
                NoiseType::Perlin => noise::domain_warp(
                    sx, sz,
                    params.warp_amplitude, params.warp_frequency,
                    |x, z| perlin.fbm(x, z, 2, params.persistence, params.lacunarity),
                    |x, z| perlin.fbm(x, z, params.octaves, params.persistence, params.lacunarity),
                ),
            }
        } else {
            match params.noise_type {
                NoiseType::Value => noise::fbm(sx, sz, params.seed, params.octaves, params.persistence, params.lacunarity),
                NoiseType::Perlin => perlin.fbm(sx, sz, params.octaves, params.persistence, params.lacunarity),
            }
        };
        n * params.height_scale
    })
}
