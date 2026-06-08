//! Terrain editor: sculpt, paint, vegetation placement.

use rustix_core::math::Vec3;
use rustix_terrain::sculpt::{SculptBrush, BrushMode};
use rustix_terrain::Heightmap;

/// Terrain editing tool modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainEditMode {
    Sculpt,
    Paint,
    Vegetation,
    Smooth,
    Flatten,
}

/// Terrain editor state.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainEditorState {
    pub mode: TerrainEditMode,
    pub brush: SculptBrush,
    pub paint_layer: usize,
    pub paint_strength: f32,
    pub vegetation_density: f32,
}

impl Default for TerrainEditorState {
    fn default() -> Self {
        Self {
            mode: TerrainEditMode::Sculpt,
            brush: SculptBrush::new(),
            paint_layer: 0,
            paint_strength: 0.5,
            vegetation_density: 1.0,
        }
    }
}

impl TerrainEditorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_brush_mode(&mut self, mode: BrushMode) {
        self.brush.mode = mode;
    }

    pub fn set_brush_radius(&mut self, radius: f32) {
        self.brush = self.brush.radius(radius);
    }

    pub fn set_brush_strength(&mut self, strength: f32) {
        self.brush = self.brush.strength(strength);
    }

    /// Apply the current brush to a heightmap at a world position.
    pub fn apply_brush(&self, heightmap: &mut Heightmap, world_scale: f32, x: f32, z: f32) {
        match self.mode {
            TerrainEditMode::Sculpt | TerrainEditMode::Smooth | TerrainEditMode::Flatten => {
                self.brush.apply(heightmap, world_scale, x, z);
            }
            TerrainEditMode::Paint => {
                // Paint logic would modify splat maps
            }
            TerrainEditMode::Vegetation => {
                // Vegetation placement logic
            }
        }
    }
}
