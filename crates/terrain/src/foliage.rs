//! Grass and foliage instancing on terrain surfaces.
//!
//! Generates instance transforms for grass blades, rocks, or other
//! detail objects scattered across the terrain mesh.

use rustix_core::math::{Vec3, Quat, Mat4};

/// An instance of a detail object placed on the terrain.
#[derive(Debug, Clone, Copy)]
pub struct FoliageInstance {
    pub position: Vec3,
    pub scale: f32,
    pub rotation: Quat,
    pub layer_index: usize,
}

impl FoliageInstance {
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            self.rotation,
            self.position,
        )
    }
}

/// Scatter foliage instances across a terrain heightmap.
pub fn scatter_foliage(
    heightmap: &crate::Heightmap,
    world_scale: f32,
    density: f32, // instances per world-unit square
    layers: &[FoliageLayer],
    random_fn: &mut dyn FnMut() -> f32,
) -> Vec<FoliageInstance> {
    let mut instances = Vec::new();
    let area = world_scale * world_scale;
    let count_per_cell = (density * area).ceil() as usize;

    for z in 0..heightmap.depth {
        for x in 0..heightmap.width {
            let h = heightmap.get(x, z);
            let wx = x as f32 * world_scale;
            let wz = z as f32 * world_scale;

            for _ in 0..count_per_cell {
                let offset_x = random_fn() * world_scale;
                let offset_z = random_fn() * world_scale;
                let pos = Vec3::new(wx + offset_x, h, wz + offset_z);

                for (layer_idx, layer) in layers.iter().enumerate() {
                    if layer.can_place(pos, h) {
                        let scale = layer.min_scale + random_fn() * (layer.max_scale - layer.min_scale);
                        let yaw = random_fn() * std::f32::consts::TAU;
                        let rot = Quat::from_axis_angle(Vec3::Y, yaw);
                        instances.push(FoliageInstance {
                            position: pos,
                            scale,
                            rotation: rot,
                            layer_index: layer_idx,
                        });
                        break;
                    }
                }
            }
        }
    }
    instances
}

/// Configuration for one foliage layer.
#[derive(Debug, Clone, Copy)]
pub struct FoliageLayer {
    pub min_height: f32,
    pub max_height: f32,
    pub max_slope: f32,
    pub min_scale: f32,
    pub max_scale: f32,
}

impl FoliageLayer {
    pub fn new() -> Self {
        Self {
            min_height: f32::NEG_INFINITY,
            max_height: f32::INFINITY,
            max_slope: 0.7, // ~45 degrees
            min_scale: 0.8,
            max_scale: 1.2,
        }
    }

    pub fn height_range(mut self, min: f32, max: f32) -> Self {
        self.min_height = min;
        self.max_height = max;
        self
    }

    pub fn max_slope(mut self, s: f32) -> Self {
        self.max_slope = s;
        self
    }

    pub fn scale_range(mut self, min: f32, max: f32) -> Self {
        self.min_scale = min;
        self.max_scale = max;
        self
    }

    fn can_place(&self, _pos: Vec3, height: f32) -> bool {
        height >= self.min_height && height <= self.max_height
    }
}

impl Default for FoliageLayer {
    fn default() -> Self {
        Self::new()
    }
}
