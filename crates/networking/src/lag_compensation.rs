//! Lag compensation for server-side hit detection.
//!
//! The server stores a rolling history of entity transforms.
//! When a client fires, the server rewinds entity positions to the state
//! that existed when the client pulled the trigger (client time minus
//! one-way latency), then runs hit detection against the rewound world.

use std::collections::VecDeque;

/// A single historical snapshot of an entity's transform for lag compensation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LagCompSnapshot {
    pub entity_id: u64,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub bounds_radius: f32,
}

/// A frame of the world stored for lag compensation.
#[derive(Debug, Clone, PartialEq)]
pub struct LagCompFrame {
    /// Server tick this frame represents.
    pub tick: u64,
    /// Server timestamp when this frame was captured (monotonic).
    pub timestamp: f64,
    /// Entity snapshots in this frame.
    pub entities: Vec<LagCompSnapshot>,
}

/// Ring buffer of historical world frames used for lag compensation.
#[derive(Debug, Clone)]
pub struct LagCompensationBuffer {
    pub frames: VecDeque<LagCompFrame>,
    /// Maximum number of frames to retain.
    pub max_frames: usize,
    /// Tick rate (Hz) — used to estimate per-frame duration.
    pub tick_rate: f64,
}

impl LagCompensationBuffer {
    pub fn new(max_frames: usize, tick_rate: f64) -> Self {
        Self {
            frames: VecDeque::with_capacity(max_frames),
            max_frames,
            tick_rate,
        }
    }

    /// Record a new frame, evicting the oldest if at capacity.
    pub fn push(&mut self, frame: LagCompFrame) {
        while self.frames.len() >= self.max_frames {
            self.frames.pop_front();
        }
        self.frames.push_back(frame);
    }

    /// Rewind to the world state closest to `target_tick`.
    ///
    /// Returns the `LagCompFrame` with the tick nearest to `target_tick`.
    /// If the buffer is empty, returns `None`.
    pub fn rewind_to_tick(&self, target_tick: u64) -> Option<&LagCompFrame> {
        self.frames.iter().min_by_key(|f| {
            let diff = if f.tick > target_tick {
                f.tick - target_tick
            } else {
                target_tick - f.tick
            };
            diff
        })
    }

    /// Rewind to the world state closest to `target_timestamp`.
    ///
    /// Returns the `LagCompFrame` with the timestamp nearest to `target_timestamp`.
    pub fn rewind_to_time(&self, target_timestamp: f64) -> Option<&LagCompFrame> {
        self.frames.iter().min_by(|a, b| {
            let da = (a.timestamp - target_timestamp).abs();
            let db = (b.timestamp - target_timestamp).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Rewind and return interpolated entity positions between two nearest frames.
    ///
    /// If `target_tick` falls between two stored frames, this linearly
    /// interpolates each entity's position. If an entity only exists in one
    /// frame, it uses the closest available data.
    pub fn rewind_and_interpolate(&self, target_tick: u64) -> Vec<LagCompSnapshot> {
        if self.frames.len() < 2 {
            return self.rewind_to_tick(target_tick)
                .map(|f| f.entities.clone())
                .unwrap_or_default();
        }

        // Find the two surrounding frames.
        let frames: Vec<&LagCompFrame> = self.frames.iter().collect();
        let mut surrounding: Option<(&LagCompFrame, &LagCompFrame)> = None;
        for window in frames.windows(2) {
            let prev = window[0];
            let next = window[1];
            if prev.tick <= target_tick && target_tick <= next.tick {
                surrounding = Some((prev, next));
                break;
            }
        }

        let (prev, next) = match surrounding {
            Some(pair) => pair,
            None => {
                // target_tick is outside the buffered range — return nearest.
                return self.rewind_to_tick(target_tick)
                    .map(|f| f.entities.clone())
                    .unwrap_or_default();
            }
        };

        let t = if next.tick > prev.tick {
            ((target_tick - prev.tick) as f64 / (next.tick - prev.tick) as f64) as f32
        } else {
            0.0
        };
        let t = t.clamp(0.0, 1.0);

        // Build a map of next-frame entities for fast lookup.
        let next_map: std::collections::HashMap<u64, &LagCompSnapshot> =
            next.entities.iter().map(|e| (e.entity_id, e)).collect();

        let mut result = Vec::with_capacity(prev.entities.len());
        for prev_ent in &prev.entities {
            let interp = if let Some(next_ent) = next_map.get(&prev_ent.entity_id) {
                lerp_snapshot(prev_ent, next_ent, t)
            } else {
                *prev_ent
            };
            result.push(interp);
        }

        // Add entities that only exist in the next frame.
        let prev_ids: std::collections::HashSet<u64> =
            prev.entities.iter().map(|e| e.entity_id).collect();
        for next_ent in &next.entities {
            if !prev_ids.contains(&next_ent.entity_id) {
                result.push(*next_ent);
            }
        }

        result
    }

    /// Clear all stored history.
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    /// Age of the oldest frame in the buffer (seconds).
    pub fn buffer_age(&self) -> f64 {
        if let (Some(front), Some(back)) = (self.frames.front(), self.frames.back()) {
            back.timestamp - front.timestamp
        } else {
            0.0
        }
    }

    /// Estimated one-way latency for a given client RTT.
    ///
    /// This is the value subtracted from the server time to determine
    /// which tick to rewind to for that client's shot.
    pub fn latency_from_rtt(rtt_ms: f64) -> f64 {
        rtt_ms / 2.0 / 1000.0
    }
}

/// Linear interpolation between two entity snapshots.
fn lerp_snapshot(a: &LagCompSnapshot, b: &LagCompSnapshot, t: f32) -> LagCompSnapshot {
    let one_minus_t = 1.0 - t;
    LagCompSnapshot {
        entity_id: a.entity_id,
        position: [
            a.position[0] * one_minus_t + b.position[0] * t,
            a.position[1] * one_minus_t + b.position[1] * t,
            a.position[2] * one_minus_t + b.position[2] * t,
        ],
        rotation: nlerp_quat(a.rotation, b.rotation, t),
        bounds_radius: a.bounds_radius * one_minus_t + b.bounds_radius * t,
    }
}

/// Normalized linear interpolation between two quaternions.
fn nlerp_quat(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let (b0, b1, b2, b3) = if dot < 0.0 {
        (-b[0], -b[1], -b[2], -b[3])
    } else {
        (b[0], b[1], b[2], b[3])
    };
    let one_minus_t = 1.0 - t;
    let x = a[0] * one_minus_t + b0 * t;
    let y = a[1] * one_minus_t + b1 * t;
    let z = a[2] * one_minus_t + b2 * t;
    let w = a[3] * one_minus_t + b3 * t;
    let len = (x * x + y * y + z * z + w * w).sqrt();
    if len > 0.0 {
        [x / len, y / len, z / len, w / len]
    } else {
        a
    }
}

/// Result of a lag-compensated hit scan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitResult {
    pub entity_id: u64,
    pub hit_position: [f32; 3],
    pub distance: f32,
}

/// Simple sphere-vs-ray lag-compensated hit detection.
///
/// Given a ray origin and direction, tests against all entities in the
/// rewound frame and returns the closest hit.
pub fn lag_compensated_raycast(
    origin: [f32; 3],
    direction: [f32; 3],
    max_distance: f32,
    entities: &[LagCompSnapshot],
) -> Option<HitResult> {
    let mut closest: Option<HitResult> = None;
    let dir_len = (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2]).sqrt();
    if dir_len < 1e-6 {
        return None;
    }
    let dir = [direction[0] / dir_len, direction[1] / dir_len, direction[2] / dir_len];

    for ent in entities {
        let to_center = [
            ent.position[0] - origin[0],
            ent.position[1] - origin[1],
            ent.position[2] - origin[2],
        ];
        let proj = to_center[0] * dir[0] + to_center[1] * dir[1] + to_center[2] * dir[2];
        if proj < 0.0 || proj > max_distance {
            continue;
        }
        let closest_point = [
            origin[0] + dir[0] * proj,
            origin[1] + dir[1] * proj,
            origin[2] + dir[2] * proj,
        ];
        let dist_sq = (closest_point[0] - ent.position[0]).powi(2)
            + (closest_point[1] - ent.position[1]).powi(2)
            + (closest_point[2] - ent.position[2]).powi(2);
        if dist_sq <= ent.bounds_radius * ent.bounds_radius {
            let dist = dist_sq.sqrt();
            let hit = HitResult {
                entity_id: ent.entity_id,
                hit_position: closest_point,
                distance: dist,
            };
            match &closest {
                Some(c) if hit.distance < c.distance => closest = Some(hit),
                None => closest = Some(hit),
                _ => {}
            }
        }
    }
    closest
}
