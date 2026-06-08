//! Erosion simulation (thermal and hydraulic).
//!
//! Simple iterative erosion that redistributes height based on slope
//! and local minima.

use crate::Heightmap;

/// Thermal erosion parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalErosionParams {
    /// Talus angle threshold (height difference per cell) above which
    /// material slides.
    pub talus_angle: f32,
    /// Fraction of excess height moved per iteration.
    pub transport_rate: f32,
    /// Number of iterations.
    pub iterations: usize,
}

impl Default for ThermalErosionParams {
    fn default() -> Self {
        Self {
            talus_angle: 1.0,
            transport_rate: 0.5,
            iterations: 20,
        }
    }
}

/// Apply thermal erosion to a heightmap.
pub fn thermal_erosion(heightmap: &mut Heightmap, params: &ThermalErosionParams) {
    let w = heightmap.width;
    let d = heightmap.depth;
    let mut deltas = vec![0.0f32; w * d];

    for _ in 0..params.iterations {
        deltas.fill(0.0);

        for z in 1..d - 1 {
            for x in 1..w - 1 {
                let idx = z * w + x;
                let h = heightmap.heights[idx];
                let mut max_diff = 0.0f32;
                let mut total_diff = 0.0f32;

                for dz in -1..=1i32 {
                    for dx in -1..=1i32 {
                        if dx == 0 && dz == 0 {
                            continue;
                        }
                        let nx = x as i32 + dx;
                        let nz = z as i32 + dz;
                        let nidx = nz as usize * w + nx as usize;
                        let diff = h - heightmap.heights[nidx];
                        if diff > params.talus_angle {
                            total_diff += diff - params.talus_angle;
                            if diff > max_diff {
                                max_diff = diff;
                            }
                        }
                    }
                }

                if total_diff > 0.0 {
                    let move_amount = max_diff.min(total_diff * params.transport_rate);
                    deltas[idx] -= move_amount;

                    for dz in -1..=1i32 {
                        for dx in -1..=1i32 {
                            if dx == 0 && dz == 0 {
                                continue;
                            }
                            let nx = x as i32 + dx;
                            let nz = z as i32 + dz;
                            let nidx = nz as usize * w + nx as usize;
                            let diff = h - heightmap.heights[nidx];
                            if diff > params.talus_angle {
                                let portion = (diff - params.talus_angle) / total_diff;
                                deltas[nidx] += move_amount * portion;
                            }
                        }
                    }
                }
            }
        }

        for i in 0..w * d {
            heightmap.heights[i] += deltas[i];
        }
    }
}

/// Hydraulic erosion parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HydraulicErosionParams {
    /// Rain amount added each iteration.
    pub rain_rate: f32,
    /// Solubility: how much soil dissolves per water unit.
    pub solubility: f32,
    /// Evaporation rate per iteration.
    pub evaporation: f32,
    /// Number of iterations.
    pub iterations: usize,
}

impl Default for HydraulicErosionParams {
    fn default() -> Self {
        Self {
            rain_rate: 0.01,
            solubility: 0.1,
            evaporation: 0.05,
            iterations: 20,
        }
    }
}

/// Apply a simplified hydraulic erosion pass.
///
/// This is a cellular-automata-style approximation rather than a full
/// pipe model.
pub fn hydraulic_erosion(heightmap: &mut Heightmap, params: &HydraulicErosionParams) {
    let w = heightmap.width;
    let d = heightmap.depth;
    let area = w * d;
    let mut water = vec![0.0f32; area];
    let mut sediment = vec![0.0f32; area];

    for _ in 0..params.iterations {
        // Rain
        for cell in &mut water {
            *cell += params.rain_rate;
        }

        // Dissolve
        for i in 0..area {
            let dissolved = water[i] * params.solubility;
            heightmap.heights[i] -= dissolved;
            sediment[i] += dissolved;
        }

        // Flow to lowest neighbor
        let mut height_deltas = vec![0.0f32; area];
        let mut sediment_deltas = vec![0.0f32; area];
        let mut water_deltas = vec![0.0f32; area];

        for z in 1..d - 1 {
            for x in 1..w - 1 {
                let idx = z * w + x;
                let h = heightmap.heights[idx];
                let mut lowest_idx = idx;
                let mut lowest_h = h;

                for dz in -1..=1i32 {
                    for dx in -1..=1i32 {
                        if dx == 0 && dz == 0 {
                            continue;
                        }
                        let nx = x as i32 + dx;
                        let nz = z as i32 + dz;
                        let nidx = nz as usize * w + nx as usize;
                        if heightmap.heights[nidx] < lowest_h {
                            lowest_h = heightmap.heights[nidx];
                            lowest_idx = nidx;
                        }
                    }
                }

                if lowest_idx != idx && water[idx] > 0.0 {
                    let flow = water[idx] * 0.5;
                    water_deltas[idx] -= flow;
                    water_deltas[lowest_idx] += flow;
                    let sed_flow = sediment[idx] * 0.5;
                    sediment_deltas[idx] -= sed_flow;
                    sediment_deltas[lowest_idx] += sed_flow;
                }
            }
        }

        for i in 0..area {
            water[i] += water_deltas[i];
            sediment[i] += sediment_deltas[i];
            water[i] *= 1.0 - params.evaporation;
            // Deposit remaining sediment
            let deposit = sediment[i] * params.evaporation;
            height_deltas[i] += deposit;
            sediment[i] -= deposit;
        }

        for i in 0..area {
            heightmap.heights[i] += height_deltas[i];
        }
    }
}
