//! Splat-map texturing for terrain.
//!
//! A splat map stores blend weights for multiple material layers.
//! Each channel (R, G, B, A) in a splat texture corresponds to one
//! material layer. Multiple splat maps can be stacked for up to 8
//! layers.

/// A single terrain material layer.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainLayer {
    pub name: String,
    /// Minimum height where this layer can appear.
    pub min_height: f32,
    /// Maximum height where this layer can appear.
    pub max_height: f32,
    /// Minimum slope (0 = flat, 1 = vertical) for this layer.
    pub min_slope: f32,
    /// Maximum slope for this layer.
    pub max_slope: f32,
    /// Base weight when conditions are met.
    pub base_weight: f32,
}

impl TerrainLayer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            min_height: f32::NEG_INFINITY,
            max_height: f32::INFINITY,
            min_slope: 0.0,
            max_slope: 1.0,
            base_weight: 1.0,
        }
    }

    pub fn height_range(mut self, min: f32, max: f32) -> Self {
        self.min_height = min;
        self.max_height = max;
        self
    }

    pub fn slope_range(mut self, min: f32, max: f32) -> Self {
        self.min_slope = min;
        self.max_slope = max;
        self
    }

    pub fn weight(mut self, w: f32) -> Self {
        self.base_weight = w;
        self
    }

    /// Compute the layer's weight at a given height and slope.
    pub fn compute_weight(&self, height: f32, slope: f32) -> f32 {
        if height < self.min_height || height > self.max_height {
            return 0.0;
        }
        if slope < self.min_slope || slope > self.max_slope {
            return 0.0;
        }
        self.base_weight
    }
}

/// A splat map that stores blend weights for up to 4 layers per pixel.
#[derive(Debug, Clone)]
pub struct SplatMap {
    pub width: usize,
    pub depth: usize,
    /// 4 weights per pixel (RGBA), stored linearly.
    pub weights: Vec<[f32; 4]>,
    /// Layer indices assigned to each of the 4 channels.
    pub layer_indices: [usize; 4],
}

impl SplatMap {
    pub fn new(width: usize, depth: usize, layer_indices: [usize; 4]) -> Self {
        Self {
            width,
            depth,
            weights: vec![[0.0; 4]; width * depth],
            layer_indices,
        }
    }

    pub fn get(&self, x: usize, z: usize) -> [f32; 4] {
        if x >= self.width || z >= self.depth {
            [0.0; 4]
        } else {
            self.weights[z * self.width + x]
        }
    }

    pub fn set(&mut self, x: usize, z: usize, weights: [f32; 4]) {
        if x < self.width && z < self.depth {
            self.weights[z * self.width + x] = weights;
        }
    }

    /// Normalize weights so they sum to 1.0 per pixel.
    pub fn normalize(&mut self) {
        for w in &mut self.weights {
            let sum: f32 = w.iter().sum();
            if sum > 0.0 {
                for c in w.iter_mut() {
                    *c /= sum;
                }
            }
        }
    }

    /// Compute splat weights from height and slope constraints for all layers.
    /// `layers` must contain at least as many entries as the highest index in `layer_indices`.
    pub fn generate_from_terrain(&mut self, heights: &[f32], slopes: &[f32]) {
        assert_eq!(heights.len(), self.width * self.depth);
        assert_eq!(slopes.len(), self.width * self.depth);
        for z in 0..self.depth {
            for x in 0..self.width {
                let idx = z * self.width + x;
                // Stub: real implementation would look up layer objects and compute weights
                let _ = (heights[idx], slopes[idx]);
            }
        }
    }
}

/// A collection of splat maps supporting up to 8 layers (two RGBA maps).
#[derive(Debug, Clone)]
pub struct SplatStack {
    pub layers: Vec<TerrainLayer>,
    pub maps: Vec<SplatMap>,
}

impl SplatStack {
    pub fn new(layers: Vec<TerrainLayer>) -> Self {
        let mut maps = Vec::new();
        for i in (0..layers.len()).step_by(4) {
            let idx = [
                i,
                i + 1,
                i + 2,
                i + 3,
            ];
            maps.push(SplatMap::new(1, 1, idx));
        }
        Self { layers, maps }
    }

    /// Resize all splat maps to match a heightmap resolution.
    pub fn resize(&mut self, width: usize, depth: usize) {
        for map in &mut self.maps {
            map.width = width;
            map.depth = depth;
            map.weights = vec![[0.0; 4]; width * depth];
        }
    }

    /// Regenerate all splat weights from height and slope data.
    pub fn generate(&mut self, heights: &[f32], slopes: &[f32], width: usize, depth: usize) {
        self.resize(width, depth);
        for z in 0..depth {
            for x in 0..width {
                let idx = z * width + x;
                let h = heights[idx];
                let s = slopes[idx];
                let mut remaining_weight = 1.0f32;

                for map in &mut self.maps {
                    let mut w = [0.0f32; 4];
                    for c in 0..4 {
                        let layer_idx = map.layer_indices[c];
                        if layer_idx < self.layers.len() && remaining_weight > 0.0 {
                            let lw = self.layers[layer_idx].compute_weight(h, s);
                            let applied = lw.min(remaining_weight);
                            w[c] = applied;
                            remaining_weight -= applied;
                        }
                    }
                    map.set(x, z, w);
                }
            }
        }
        for map in &mut self.maps {
            map.normalize();
        }
    }
}
