//! Influence maps for tactical decision making.
//!
//! A 2D grid where each cell stores floating-point influence values
//! that decay with distance. Used for territory control, threat
//! awareness, and strategic positioning.

/// A 2D influence map backed by a flat Vec.
#[derive(Debug, Clone, PartialEq)]
pub struct InfluenceMap {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    pub origin: [f32; 2],
    pub values: Vec<f32>,
}

impl InfluenceMap {
    pub fn new(width: usize, height: usize, cell_size: f32, origin: [f32; 2]) -> Self {
        Self {
            width,
            height,
            cell_size,
            origin,
            values: vec![0.0; width * height],
        }
    }

    pub fn clear(&mut self) {
        self.values.fill(0.0);
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height {
            self.values[self.idx(x, y)]
        } else {
            0.0
        }
    }

    pub fn set(&mut self, x: usize, y: usize, value: f32) {
        if x < self.width && y < self.height {
            let i = self.idx(x, y);
            self.values[i] = value;
        }
    }

    pub fn add(&mut self, x: usize, y: usize, delta: f32) {
        if x < self.width && y < self.height {
            let i = self.idx(x, y);
            self.values[i] += delta;
        }
    }

    /// Convert a world position to grid coordinates.
    pub fn world_to_grid(&self, wx: f32, wy: f32) -> (usize, usize) {
        let gx = ((wx - self.origin[0]) / self.cell_size).floor() as isize;
        let gy = ((wy - self.origin[1]) / self.cell_size).floor() as isize;
        (
            gx.clamp(0, self.width as isize - 1) as usize,
            gy.clamp(0, self.height as isize - 1) as usize,
        )
    }

    /// Convert grid coordinates to world position (cell center).
    pub fn grid_to_world(&self, gx: usize, gy: usize) -> (f32, f32) {
        (
            self.origin[0] + (gx as f32 + 0.5) * self.cell_size,
            self.origin[1] + (gy as f32 + 0.5) * self.cell_size,
        )
    }

    /// Stamp a radial influence at a world position.
    /// Influence falls off linearly from `strength` at the center to 0 at `radius`.
    pub fn stamp_influence(&mut self, wx: f32, wy: f32, strength: f32, radius: f32) {
        let (cx, cy) = self.world_to_grid(wx, wy);
        let cells = (radius / self.cell_size).ceil() as isize;

        for dy in -cells..=cells {
            for dx in -cells..=cells {
                let gx = cx as isize + dx;
                let gy = cy as isize + dy;
                if gx < 0 || gy < 0 {
                    continue;
                }
                let gx = gx as usize;
                let gy = gy as usize;
                if gx >= self.width || gy >= self.height {
                    continue;
                }
                let (cell_wx, cell_wy) = self.grid_to_world(gx, gy);
                let dist = ((cell_wx - wx).powi(2) + (cell_wy - wy).powi(2)).sqrt();
                if dist <= radius {
                    let factor = 1.0 - dist / radius;
                    self.add(gx, gy, strength * factor);
                }
            }
        }
    }

    /// Apply a global decay factor (0..1) to all cells.
    pub fn decay(&mut self, factor: f32) {
        for v in &mut self.values {
            *v *= factor;
        }
    }

    /// Clamp all values to a range.
    pub fn clamp(&mut self, min: f32, max: f32) {
        for v in &mut self.values {
            *v = v.clamp(min, max);
        }
    }

    /// Find the grid cell with the highest influence.
    pub fn highest_cell(&self) -> Option<(usize, usize, f32)> {
        self.values
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, &v)| {
                let x = i % self.width;
                let y = i / self.width;
                (x, y, v)
            })
    }

    /// Find the grid cell with the lowest influence.
    pub fn lowest_cell(&self) -> Option<(usize, usize, f32)> {
        self.values
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, &v)| {
                let x = i % self.width;
                let y = i / self.width;
                (x, y, v)
            })
    }

    /// Add another influence map cell-by-cell.
    pub fn add_map(&mut self, other: &InfluenceMap) {
        let w = self.width.min(other.width);
        let h = self.height.min(other.height);
        for y in 0..h {
            for x in 0..w {
                let i = self.idx(x, y);
                let j = other.idx(x, y);
                self.values[i] += other.values[j];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stamp_and_highest() {
        let mut map = InfluenceMap::new(10, 10, 1.0, [0.0, 0.0]);
        map.stamp_influence(5.0, 5.0, 10.0, 2.0);
        let (x, y, v) = map.highest_cell().unwrap();
        assert_eq!(x, 5);
        assert_eq!(y, 5);
        assert!(v > 0.0);
    }

    #[test]
    fn test_decay() {
        let mut map = InfluenceMap::new(2, 2, 1.0, [0.0, 0.0]);
        map.set(0, 0, 10.0);
        map.decay(0.5);
        assert!((map.get(0, 0) - 5.0).abs() < 0.001);
    }
}
