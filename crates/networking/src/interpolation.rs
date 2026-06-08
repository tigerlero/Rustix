//! Entity interpolation for remote players using snapshot buffering.
//!
//! Remote entities are rendered between two past snapshots to smooth
//! out network jitter. The local player is predicted, not interpolated.

use std::collections::VecDeque;

/// A single server snapshot at a specific tick.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot<T: Clone> {
    pub tick: u64,
    pub timestamp: f64,
    pub state: T,
}

/// A ring buffer of snapshots used to interpolate remote entity state.
#[derive(Debug, Clone)]
pub struct SnapshotBuffer<T: Clone> {
    pub snapshots: VecDeque<Snapshot<T>>,
    /// How far back (in seconds) to interpolate behind the latest snapshot.
    pub interpolation_delay: f64,
    pub max_size: usize,
}

impl<T: Clone> SnapshotBuffer<T> {
    pub fn new(interpolation_delay: f64, max_size: usize) -> Self {
        Self {
            snapshots: VecDeque::with_capacity(max_size),
            interpolation_delay,
            max_size,
        }
    }

    /// Insert a new snapshot, maintaining tick order.
    pub fn push(&mut self, snapshot: Snapshot<T>) {
        // Remove old snapshots to stay within max size.
        while self.snapshots.len() >= self.max_size {
            self.snapshots.pop_front();
        }
        // Ensure ordering by tick — discard out-of-order older snapshots.
        if let Some(back) = self.snapshots.back() {
            if snapshot.tick <= back.tick {
                // Out of order or duplicate — ignore.
                return;
            }
        }
        self.snapshots.push_back(snapshot);
    }

    /// Find the two snapshots surrounding `render_time`.
    fn find_surrounding(&self, render_time: f64) -> Option<(&Snapshot<T>, &Snapshot<T>)> {
        let snapshots: Vec<&Snapshot<T>> = self.snapshots.iter().collect();
        for window in snapshots.windows(2) {
            let prev = window[0];
            let next = window[1];
            if prev.timestamp <= render_time && render_time <= next.timestamp {
                return Some((prev, next));
            }
        }
        None
    }

    /// Interpolate state at the current render time.
    ///
    /// Returns `None` if there are not enough snapshots buffered yet.
    pub fn interpolate(&self, current_time: f64) -> Option<T>
    where
        T: Interpolatable,
    {
        if self.snapshots.len() < 2 {
            return self.snapshots.back().map(|s| s.state.clone());
        }

        let render_time = current_time - self.interpolation_delay;
        let (prev, next) = self.find_surrounding(render_time)?;

        let t = if next.timestamp > prev.timestamp {
            ((render_time - prev.timestamp) / (next.timestamp - prev.timestamp)) as f32
        } else {
            0.0
        };
        let t = t.clamp(0.0, 1.0);

        Some(prev.state.interpolate(&next.state, t))
    }
}

/// Trait for types that can be linearly interpolated.
pub trait Interpolatable: Clone {
    fn interpolate(&self, other: &Self, t: f32) -> Self;
}

/// Simple 3D position that can be interpolated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterpPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Interpolatable for InterpPosition {
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        let one_minus_t = 1.0 - t;
        Self {
            x: self.x * one_minus_t + other.x * t,
            y: self.y * one_minus_t + other.y * t,
            z: self.z * one_minus_t + other.z * t,
        }
    }
}

/// Interpolatable entity state for remote players.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterpEntityState {
    pub position: InterpPosition,
    pub rotation: [f32; 4], // Quaternion
}

impl Interpolatable for InterpEntityState {
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            position: self.position.interpolate(&other.position, t),
            rotation: slerp_quat(self.rotation, other.rotation, t),
        }
    }
}

/// Spherical linear interpolation between two quaternions.
fn slerp_quat(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    // Simple nlerp approximation (fast, good for small angles).
    // For exact slerp use `Quat::slerp` from glam.
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
