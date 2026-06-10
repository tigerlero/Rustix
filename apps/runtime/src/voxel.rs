//! Voxel / Minecraft-like world system for Rustix.
//!
//! Provides block types, chunk meshing, terrain generation, and first-person
//! block interaction (break / place).

use rustix_core::math::{Vec3, Mat4};
use rustix_render::mesh::{Mesh, Vertex};
use rustix_render::Renderer;

pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_HEIGHT: usize = 32;

/// Block types in the voxel world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockType {
    #[default]
    Air,
    Grass,
    Dirt,
    Stone,
    Wood,
    Leaves,
    Sand,
    Water,
    Bedrock,
}

impl BlockType {
    pub fn is_solid(self) -> bool {
        !matches!(self, BlockType::Air | BlockType::Water)
    }

    pub fn base_color(self) -> Vec3 {
        match self {
            BlockType::Air => Vec3::ZERO,
            BlockType::Grass => Vec3::new(0.35, 0.65, 0.25),
            BlockType::Dirt => Vec3::new(0.55, 0.35, 0.22),
            BlockType::Stone => Vec3::new(0.5, 0.5, 0.52),
            BlockType::Wood => Vec3::new(0.55, 0.35, 0.15),
            BlockType::Leaves => Vec3::new(0.15, 0.45, 0.15),
            BlockType::Sand => Vec3::new(0.76, 0.7, 0.5),
            BlockType::Water => Vec3::new(0.2, 0.4, 0.7),
            BlockType::Bedrock => Vec3::new(0.3, 0.3, 0.3),
        }
    }

    pub fn roughness(self) -> f32 {
        match self {
            BlockType::Water => 0.1,
            BlockType::Stone | BlockType::Bedrock => 0.9,
            BlockType::Sand => 0.95,
            _ => 0.8,
        }
    }

    pub fn metallic(self) -> f32 {
        0.0
    }
}

/// A single chunk of the voxel world.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub blocks: [[[BlockType; CHUNK_SIZE]; CHUNK_HEIGHT]; CHUNK_SIZE],
    pub chunk_x: i32,
    pub chunk_z: i32,
}

impl Chunk {
    pub fn new(chunk_x: i32, chunk_z: i32) -> Self {
        Self {
            blocks: [[[BlockType::Air; CHUNK_SIZE]; CHUNK_HEIGHT]; CHUNK_SIZE],
            chunk_x,
            chunk_z,
        }
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockType {
        if x >= CHUNK_SIZE || y >= CHUNK_HEIGHT || z >= CHUNK_SIZE {
            return BlockType::Air;
        }
        self.blocks[x][y][z]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, block: BlockType) {
        if x < CHUNK_SIZE && y < CHUNK_HEIGHT && z < CHUNK_SIZE {
            self.blocks[x][y][z] = block;
        }
    }
}

/// Simple pseudo-random noise for terrain height.
pub fn hash_noise(x: i32, z: i32, seed: i32) -> f32 {
    let mut h = x.wrapping_mul(374761393).wrapping_add(z.wrapping_mul(668265263)).wrapping_add(seed.wrapping_mul(12345));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h = h ^ (h >> 16);
    (h & 0x7FFFFFFF) as f32 / 2147483647.0
}

pub fn smooth_noise(x: i32, z: i32) -> f32 {
    let _fx = x as f32;
    let _fz = z as f32;
    let n = hash_noise(x, z, 0)
        + hash_noise(x + 1, z, 1) * 0.5
        + hash_noise(x, z + 1, 2) * 0.5
        + hash_noise(x - 1, z, 3) * 0.5
        + hash_noise(x, z - 1, 4) * 0.5;
    n / 3.0
}

/// Generate terrain height at a given world x,z.
pub fn terrain_height(world_x: i32, world_z: i32) -> i32 {
    let base = 8.0;
    let variation = smooth_noise(world_x, world_z) * 8.0;
    let hills = (smooth_noise(world_x / 3, world_z / 3) * 6.0).sin() * 3.0;
    (base + variation + hills).max(1.0).min(CHUNK_HEIGHT as f32 - 2.0) as i32
}

/// Fill a chunk with terrain.
pub fn generate_chunk_terrain(chunk: &mut Chunk) {
    for x in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            let world_x = chunk.chunk_x * CHUNK_SIZE as i32 + x as i32;
            let world_z = chunk.chunk_z * CHUNK_SIZE as i32 + z as i32;
            let h = terrain_height(world_x, world_z);

            for y in 0..CHUNK_HEIGHT {
                let block = if y == 0 {
                    BlockType::Bedrock
                } else if (y as i32) < h - 3 {
                    BlockType::Stone
                } else if (y as i32) < h {
                    BlockType::Dirt
                } else if (y as i32) == h {
                    BlockType::Grass
                } else {
                    BlockType::Air
                };
                chunk.set(x, y, z, block);
            }

            // Occasional trees
            if h >= 4 && h < CHUNK_HEIGHT as i32 - 6 {
                let tree_noise = hash_noise(world_x, world_z, 99);
                if tree_noise > 0.96 {
                    // Trunk
                    for ty in (h + 1)..=(h + 4).min(CHUNK_HEIGHT as i32 - 1) {
                        chunk.set(x, ty as usize, z, BlockType::Wood);
                    }
                    // Leaves
                    let leaf_y = (h + 4).min(CHUNK_HEIGHT as i32 - 1) as usize;
                    for lx in x.saturating_sub(1)..=(x + 1).min(CHUNK_SIZE - 1) {
                        for lz in z.saturating_sub(1)..=(z + 1).min(CHUNK_SIZE - 1) {
                            for ly in leaf_y.saturating_sub(1)..=leaf_y + 1 {
                                if ly < CHUNK_HEIGHT && chunk.get(lx, ly, lz) == BlockType::Air {
                                    chunk.set(lx, ly, lz, BlockType::Leaves);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Build a mesh for a chunk, only drawing exposed faces.
pub fn build_chunk_mesh(renderer: &Renderer, chunk: &Chunk, name: &str) -> Result<Mesh, rustix_render::RenderError> {
    let mut verts: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    let mut add_face = |pos: [f32; 3], normal: [f32; 3], color: Vec3| {
        let base = verts.len() as u16;
        let s = 1.0;
        let offsets = match normal {
            [0.0, 1.0, 0.0] => ([0.0, s, 0.0], [s, s, 0.0], [s, s, s], [0.0, s, s]), // top
            [0.0, -1.0, 0.0] => ([0.0, 0.0, 0.0], [s, 0.0, 0.0], [s, 0.0, s], [0.0, 0.0, s]), // bottom
            [0.0, 0.0, 1.0] => ([0.0, 0.0, s], [s, 0.0, s], [s, s, s], [0.0, s, s]), // front
            [0.0, 0.0, -1.0] => ([s, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, s, 0.0], [s, s, 0.0]), // back
            [1.0, 0.0, 0.0] => ([s, 0.0, s], [s, 0.0, 0.0], [s, s, 0.0], [s, s, s]), // right
            [-1.0, 0.0, 0.0] => ([0.0, 0.0, 0.0], [0.0, 0.0, s], [0.0, s, s], [0.0, s, 0.0]), // left
            _ => ([0.0, 0.0, 0.0], [s, 0.0, 0.0], [s, s, 0.0], [0.0, s, 0.0]),
        };
        let o = pos;
        let p0 = [o[0] + offsets.0[0], o[1] + offsets.0[1], o[2] + offsets.0[2]];
        let p1 = [o[0] + offsets.1[0], o[1] + offsets.1[1], o[2] + offsets.1[2]];
        let p2 = [o[0] + offsets.2[0], o[1] + offsets.2[1], o[2] + offsets.2[2]];
        let p3 = [o[0] + offsets.3[0], o[1] + offsets.3[1], o[2] + offsets.3[2]];

        verts.push(Vertex { position: p0, normal });
        verts.push(Vertex { position: p1, normal });
        verts.push(Vertex { position: p2, normal });
        verts.push(Vertex { position: p3, normal });

        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);
        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 3);
    };

    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_SIZE {
                let block = chunk.get(x, y, z);
                if !block.is_solid() {
                    continue;
                }
                let pos = [
                    (chunk.chunk_x * CHUNK_SIZE as i32 + x as i32) as f32,
                    y as f32,
                    (chunk.chunk_z * CHUNK_SIZE as i32 + z as i32) as f32,
                ];
                let color = block.base_color();

                // Top face
                if y + 1 >= CHUNK_HEIGHT || !chunk.get(x, y + 1, z).is_solid() {
                    add_face(pos, [0.0, 1.0, 0.0], color);
                }
                // Bottom face
                if y == 0 || !chunk.get(x, y.saturating_sub(1), z).is_solid() {
                    add_face(pos, [0.0, -1.0, 0.0], color);
                }
                // Front face (+Z)
                if z + 1 >= CHUNK_SIZE || !chunk.get(x, y, z + 1).is_solid() {
                    add_face(pos, [0.0, 0.0, 1.0], color);
                }
                // Back face (-Z)
                if z == 0 || !chunk.get(x, y, z.saturating_sub(1)).is_solid() {
                    add_face(pos, [0.0, 0.0, -1.0], color);
                }
                // Right face (+X)
                if x + 1 >= CHUNK_SIZE || !chunk.get(x + 1, y, z).is_solid() {
                    add_face(pos, [1.0, 0.0, 0.0], color);
                }
                // Left face (-X)
                if x == 0 || !chunk.get(x.saturating_sub(1), y, z).is_solid() {
                    add_face(pos, [-1.0, 0.0, 0.0], color);
                }
            }
        }
    }

    if verts.is_empty() {
        // Empty chunk: create a tiny dummy mesh so we don't fail
        verts.push(Vertex { position: [0.0, 0.0, 0.0], normal: [0.0, 1.0, 0.0] });
    }

    let ib = if indices.is_empty() { None } else { Some((indices.as_slice(), indices.len() as u32)) };
    Mesh::new(renderer, name, bytemuck::cast_slice(&verts), verts.len() as u32, ib)
}

/// World-space raycast against solid blocks in a chunk.
/// Returns the block position hit and the normal of the face hit.
pub fn raycast_block(chunk: &Chunk, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<([i32; 3], [i32; 3])> {
    let step = 0.05;
    let mut t = 0.0;
    let mut prev = [i32::MAX; 3];
    while t < max_dist {
        let p = origin + dir * t;
        let bx = p.x.floor() as i32 - chunk.chunk_x * CHUNK_SIZE as i32;
        let by = p.y.floor() as i32;
        let bz = p.z.floor() as i32 - chunk.chunk_z * CHUNK_SIZE as i32;

        if bx >= 0 && by >= 0 && bz >= 0
            && (bx as usize) < CHUNK_SIZE
            && (by as usize) < CHUNK_HEIGHT
            && (bz as usize) < CHUNK_SIZE
        {
            let block = chunk.get(bx as usize, by as usize, bz as usize);
            if block.is_solid() {
                let normal = if prev[0] != bx {
                    if prev[0] < bx { [-1, 0, 0] } else { [1, 0, 0] }
                } else if prev[1] != by {
                    if prev[1] < by { [0, -1, 0] } else { [0, 1, 0] }
                } else if prev[2] != bz {
                    if prev[2] < bz { [0, 0, -1] } else { [0, 0, 1] }
                } else {
                    [0, 1, 0]
                };
                return Some(([bx, by, bz], normal));
            }
            prev = [bx, by, bz];
        }
        t += step;
    }
    None
}
