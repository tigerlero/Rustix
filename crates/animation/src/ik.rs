//! Inverse Kinematics solver using Cyclic Coordinate Descent (CCD).

use rustix_core::math::{Vec3, Quat};

/// A single joint in an IK chain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IkJoint {
    pub position: Vec3,
    pub rotation: Quat,
    pub length: f32,
}

/// CCD IK solver for positioning an end-effector at a target.
pub struct CcdIkSolver {
    pub max_iterations: usize,
    pub tolerance: f32,
}

impl Default for CcdIkSolver {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            tolerance: 0.001,
        }
    }
}

impl CcdIkSolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Solve the IK chain so the end-effector reaches `target`.
    ///
    /// `chain` is a slice of joints ordered root → tip. Each joint's `length`
    /// is the distance to the next joint. The solver modifies joint rotations
    /// in-place to reach the target.
    ///
    /// Returns `true` if the target was reached within tolerance.
    pub fn solve(&self, chain: &mut [IkJoint], target: Vec3) -> bool {
        if chain.is_empty() {
            return false;
        }

        // Forward-kinematics to compute current end-effector position
        let mut positions = vec![chain[0].position];
        for i in 0..chain.len().saturating_sub(1) {
            let next_pos = positions[i] + chain[i].rotation * Vec3::Y * chain[i].length;
            positions.push(next_pos);
        }
        let end_effector = positions.last().copied().unwrap_or(chain[0].position);

        if end_effector.distance(target) < self.tolerance {
            return true;
        }

        for _ in 0..self.max_iterations {
            // Backward pass: from tip toward root
            for i in (0..chain.len().saturating_sub(1)).rev() {
                let current_end = positions.last().copied().unwrap_or(chain[0].position);
                if current_end.distance(target) < self.tolerance {
                    return true;
                }

                let to_end = current_end - positions[i];
                let to_target = target - positions[i];

                if to_end.length_squared() < 1e-12 || to_target.length_squared() < 1e-12 {
                    continue;
                }

                let axis = to_end.cross(to_target);
                if axis.length_squared() < 1e-12 {
                    continue;
                }

                let angle = to_end.angle_between(to_target);
                let delta = Quat::from_axis_angle(axis.normalize(), angle);
                chain[i].rotation = (delta * chain[i].rotation).normalize();

                // Recompute positions from this joint forward
                for j in i..chain.len().saturating_sub(1) {
                    positions[j + 1] = positions[j] + chain[j].rotation * Vec3::Y * chain[j].length;
                }
            }
        }

        let final_end = positions.last().copied().unwrap_or(chain[0].position);
        final_end.distance(target) < self.tolerance
    }
}
