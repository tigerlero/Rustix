//! Physically-based terrain materials.
//!
//! Defines per-layer PBR properties that a renderer can use when
//! sampling terrain splat textures.

/// PBR material properties for a single terrain layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainMaterial {
    /// Albedo tint (multiplied with splat diffuse texture).
    pub albedo: [f32; 4],
    /// Perceptual roughness (0 = mirror, 1 = matte).
    pub roughness: f32,
    /// Ambient occlusion factor.
    pub ao: f32,
    /// Metalness (0 = dielectric, 1 = metal).
    pub metalness: f32,
    /// Normal map strength.
    pub normal_strength: f32,
}

impl Default for TerrainMaterial {
    fn default() -> Self {
        Self {
            albedo: [1.0, 1.0, 1.0, 1.0],
            roughness: 0.8,
            ao: 1.0,
            metalness: 0.0,
            normal_strength: 1.0,
        }
    }
}

impl TerrainMaterial {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn albedo(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.albedo = [r, g, b, a];
        self
    }

    pub fn roughness(mut self, v: f32) -> Self {
        self.roughness = v.clamp(0.0, 1.0);
        self
    }

    pub fn ao(mut self, v: f32) -> Self {
        self.ao = v.clamp(0.0, 1.0);
        self
    }

    pub fn metalness(mut self, v: f32) -> Self {
        self.metalness = v.clamp(0.0, 1.0);
        self
    }

    pub fn normal_strength(mut self, v: f32) -> Self {
        self.normal_strength = v;
        self
    }
}

/// A palette of terrain materials indexed by layer.
#[derive(Debug, Clone)]
pub struct TerrainMaterialPalette {
    pub materials: Vec<TerrainMaterial>,
}

impl TerrainMaterialPalette {
    pub fn new(materials: Vec<TerrainMaterial>) -> Self {
        Self { materials }
    }

    pub fn get(&self, layer_index: usize) -> &TerrainMaterial {
        self.materials.get(layer_index).unwrap_or(&DEFAULT_TERRAIN_MATERIAL)
    }
}

const DEFAULT_TERRAIN_MATERIAL: TerrainMaterial = TerrainMaterial {
    albedo: [1.0, 1.0, 1.0, 1.0],
    roughness: 0.8,
    ao: 1.0,
    metalness: 0.0,
    normal_strength: 1.0,
};
