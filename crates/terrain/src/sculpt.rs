//! Real-time sculpting brush for terrain editing.
//!
//! Provides radial brushes that raise, lower, or flatten heightmap
//! values within a world-space radius.

use crate::Heightmap;

/// Brush operation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushMode {
    Raise,
    Lower,
    Flatten,
    Smooth,
}

/// A sculpting brush that modifies a heightmap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SculptBrush {
    pub radius: f32,
    pub strength: f32,
    pub mode: BrushMode,
    pub falloff: f32, // 0 = linear, 1 = smoothstep
}

impl Default for SculptBrush {
    fn default() -> Self {
        Self {
            radius: 5.0,
            strength: 1.0,
            mode: BrushMode::Raise,
            falloff: 0.5,
        }
    }
}

impl SculptBrush {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r.max(0.1);
        self
    }

    pub fn strength(mut self, s: f32) -> Self {
        self.strength = s;
        self
    }

    pub fn mode(mut self, m: BrushMode) -> Self {
        self.mode = m;
        self
    }

    /// Apply the brush at a world position onto the heightmap.
    pub fn apply(&self, heightmap: &mut Heightmap, world_scale: f32, center_x: f32, center_z: f32) {
        let cx = (center_x / world_scale).round() as isize;
        let cz = (center_z / world_scale).round() as isize;
        let cells = (self.radius / world_scale).ceil() as isize;

        for dz in -cells..=cells {
            for dx in -cells..=cells {
                let gx = cx + dx;
                let gz = cz + dz;
                if gx < 0 || gz < 0 {
                    continue;
                }
                let gx = gx as usize;
                let gz = gz as usize;
                if gx >= heightmap.width || gz >= heightmap.depth {
                    continue;
                }

                let wx = gx as f32 * world_scale;
                let wz = gz as f32 * world_scale;
                let dist = ((wx - center_x).powi(2) + (wz - center_z).powi(2)).sqrt();
                if dist > self.radius {
                    continue;
                }

                let t = 1.0 - dist / self.radius;
                let factor = if self.falloff > 0.0 {
                    let s = t * t * (3.0 - 2.0 * t);
                    t + (s - t) * self.falloff
                } else {
                    t
                };

                let idx = gz * heightmap.width + gx;
                match self.mode {
                    BrushMode::Raise => {
                        heightmap.heights[idx] += self.strength * factor;
                    }
                    BrushMode::Lower => {
                        heightmap.heights[idx] -= self.strength * factor;
                    }
                    BrushMode::Flatten => {
                        let target = self.strength; // reinterpret strength as target height
                        let delta = target - heightmap.heights[idx];
                        heightmap.heights[idx] += delta * factor;
                    }
                    BrushMode::Smooth => {
                        let avg = self.neighbor_average(heightmap, gx, gz);
                        let delta = avg - heightmap.heights[idx];
                        heightmap.heights[idx] += delta * self.strength * factor;
                    }
                }
            }
        }
    }

    fn neighbor_average(&self, heightmap: &Heightmap, x: usize, z: usize) -> f32 {
        let mut sum = 0.0f32;
        let mut count = 0u32;
        for dz in -1..=1i32 {
            for dx in -1..=1i32 {
                let nx = x as i32 + dx;
                let nz = z as i32 + dz;
                if nx >= 0 && nz >= 0 {
                    let nx = nx as usize;
                    let nz = nz as usize;
                    if nx < heightmap.width && nz < heightmap.depth {
                        sum += heightmap.get(nx, nz);
                        count += 1;
                    }
                }
            }
        }
        if count > 0 {
            sum / count as f32
        } else {
            heightmap.get(x, z)
        }
    }
}
