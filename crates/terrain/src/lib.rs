use rustix_core::math::Vec3;

/// Simple value noise for terrain height generation.
pub mod noise {
    use std::f32;

    fn hash(n: u32) -> u32 {
        let mut x = n;
        x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3bu32);
        x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3bu32);
        (x >> 16) ^ x
    }

    fn smooth(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }

    fn val(x: i32, z: i32, seed: u32) -> f32 {
        let h = hash((x.wrapping_add(0x9e3779b9u32 as i32) as u32).wrapping_mul(0x85ebca6bu32)
            .wrapping_add(z.wrapping_add(0x9e3779b9u32 as i32) as u32)
            .wrapping_add(seed));
        (h as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    pub fn value(x: f32, z: f32, seed: u32) -> f32 {
        let ix = x.floor() as i32;
        let iz = z.floor() as i32;
        let fx = smooth(x - ix as f32);
        let fz = smooth(z - iz as f32);

        let v00 = val(ix, iz, seed);
        let v10 = val(ix + 1, iz, seed);
        let v01 = val(ix, iz + 1, seed);
        let v11 = val(ix + 1, iz + 1, seed);

        let x0 = v00 + (v10 - v00) * fx;
        let x1 = v01 + (v11 - v01) * fx;
        x0 + (x1 - x0) * fz
    }

    pub fn fbm(x: f32, z: f32, seed: u32, octaves: u32, persistence: f32, lacunarity: f32) -> f32 {
        let mut total = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = 1.0;
        let mut max_value = 0.0;
        for _ in 0..octaves {
            total += value(x * frequency, z * frequency, seed) * amplitude;
            max_value += amplitude;
            amplitude *= persistence;
            frequency *= lacunarity;
        }
        total / max_value
    }
}

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
        }
    }
}

/// Generate a heightmap from terrain parameters.
pub fn generate_heightmap(params: &TerrainParams) -> Heightmap {
    Heightmap::from_fn(params.width, params.depth, |x, z| {
        let n = noise::fbm(x / params.width as f32 * 4.0, z / params.depth as f32 * 4.0,
            params.seed, params.octaves, params.persistence, params.lacunarity);
        n * params.height_scale
    })
}
