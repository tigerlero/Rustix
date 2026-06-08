//! Water plane and shoreline detection.
//!
//! Utilities for finding shorelines and water-covered areas on a
//! terrain heightmap.

use crate::Heightmap;

/// Detect shoreline cells where the terrain height is near the
/// water level.
pub fn find_shoreline(
    heightmap: &Heightmap,
    water_level: f32,
    tolerance: f32,
) -> Vec<(usize, usize)> {
    let mut shore = Vec::new();
    for z in 0..heightmap.depth {
        for x in 0..heightmap.width {
            let h = heightmap.get(x, z);
            if (h - water_level).abs() <= tolerance {
                shore.push((x, z));
            }
        }
    }
    shore
}

/// Flood-fill to find all connected water cells (height < level).
pub fn find_water_body(
    heightmap: &Heightmap,
    water_level: f32,
    start_x: usize,
    start_z: usize,
) -> Vec<(usize, usize)> {
    let mut body = Vec::new();
    let mut visited = vec![false; heightmap.width * heightmap.depth];
    let mut stack = vec![(start_x, start_z)];

    while let Some((x, z)) = stack.pop() {
        if x >= heightmap.width || z >= heightmap.depth {
            continue;
        }
        let idx = z * heightmap.width + x;
        if visited[idx] || heightmap.heights[idx] >= water_level {
            continue;
        }
        visited[idx] = true;
        body.push((x, z));

        if x > 0 {
            stack.push((x - 1, z));
        }
        if x + 1 < heightmap.width {
            stack.push((x + 1, z));
        }
        if z > 0 {
            stack.push((x, z - 1));
        }
        if z + 1 < heightmap.depth {
            stack.push((x, z + 1));
        }
    }

    body
}

/// Compute shoreline length (in cells) and max water depth for a
/// given water level.
pub fn water_stats(heightmap: &Heightmap, water_level: f32) -> (usize, f32) {
    let mut shoreline_len = 0usize;
    let mut max_depth = 0.0f32;

    for z in 0..heightmap.depth {
        for x in 0..heightmap.width {
            let h = heightmap.get(x, z);
            if h < water_level {
                let depth = water_level - h;
                if depth > max_depth {
                    max_depth = depth;
                }
                // Check if any neighbor is above water
                let mut is_shore = false;
                for dz in -1..=1i32 {
                    for dx in -1..=1i32 {
                        if dx == 0 && dz == 0 {
                            continue;
                        }
                        let nx = x as i32 + dx;
                        let nz = z as i32 + dz;
                        if nx >= 0 && nz >= 0 {
                            let nx = nx as usize;
                            let nz = nz as usize;
                            if nx < heightmap.width && nz < heightmap.depth {
                                if heightmap.get(nx, nz) >= water_level {
                                    is_shore = true;
                                }
                            }
                        }
                    }
                }
                if is_shore {
                    shoreline_len += 1;
                }
            }
        }
    }

    (shoreline_len, max_depth)
}
