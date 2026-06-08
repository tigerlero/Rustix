//! Sensor system for AI agents: vision cone and hearing radius.
//!
//! Provides spatial queries that AI systems can use to detect
//! enemies, allies, or points of interest.

use rustix_core::math::Vec3;

/// Vision sensor: a directional cone in front of the agent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisionCone {
    pub origin: Vec3,
    pub forward: Vec3,
    pub fov_deg: f32,
    pub max_distance: f32,
}

impl VisionCone {
    pub fn new(origin: Vec3, forward: Vec3, fov_deg: f32, max_distance: f32) -> Self {
        Self {
            origin,
            forward: forward.normalize_or_zero(),
            fov_deg,
            max_distance,
        }
    }

    /// Check if `target` is inside this vision cone.
    pub fn can_see(&self, target: Vec3) -> bool {
        let to_target = target - self.origin;
        let dist_sq = to_target.length_squared();
        if dist_sq > self.max_distance * self.max_distance {
            return false;
        }
        if dist_sq < 1e-6 {
            return true; // target is at the origin
        }
        let to_target_norm = to_target / dist_sq.sqrt();
        let cos_half_fov = (self.fov_deg.to_radians() * 0.5).cos();
        to_target_norm.dot(self.forward) >= cos_half_fov
    }

    /// Filter a list of candidate positions and return those visible
    /// from this cone.
    pub fn visible_candidates(&self, candidates: &[Vec3]) -> Vec<Vec3> {
        candidates.iter().copied().filter(|&c| self.can_see(c)).collect()
    }
}

/// Hearing sensor: a spherical radius around the agent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HearingRadius {
    pub origin: Vec3,
    pub radius: f32,
}

impl HearingRadius {
    pub fn new(origin: Vec3, radius: f32) -> Self {
        Self { origin, radius }
    }

    /// Check if `sound_source` is within hearing range.
    pub fn can_hear(&self, sound_source: Vec3, sound_radius: f32) -> bool {
        let distance = (sound_source - self.origin).length();
        distance <= self.radius + sound_radius
    }

    /// Filter a list of sound sources, returning those audible.
    /// Each candidate is `(position, sound_radius)`.
    pub fn audible_candidates(&self, candidates: &[(Vec3, f32)]) -> Vec<Vec3> {
        candidates
            .iter()
            .filter(|&&(pos, radius)| self.can_hear(pos, radius))
            .map(|&(pos, _)| pos)
            .collect()
    }
}

/// Combined AI sensor suite attached to an agent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentSensor {
    pub vision: Option<VisionCone>,
    pub hearing: Option<HearingRadius>,
}

impl AgentSensor {
    pub fn new() -> Self {
        Self {
            vision: None,
            hearing: None,
        }
    }

    pub fn with_vision(mut self, origin: Vec3, forward: Vec3, fov_deg: f32, max_distance: f32) -> Self {
        self.vision = Some(VisionCone::new(origin, forward, fov_deg, max_distance));
        self
    }

    pub fn with_hearing(mut self, origin: Vec3, radius: f32) -> Self {
        self.hearing = Some(HearingRadius::new(origin, radius));
        self
    }

    /// Update the sensor positions to match a new agent transform.
    pub fn set_position(&mut self, position: Vec3) {
        if let Some(ref mut v) = self.vision {
            v.origin = position;
        }
        if let Some(ref mut h) = self.hearing {
            h.origin = position;
        }
    }

    /// Update the vision forward direction.
    pub fn set_forward(&mut self, forward: Vec3) {
        if let Some(ref mut v) = self.vision {
            v.forward = forward.normalize_or_zero();
        }
    }
}

impl Default for AgentSensor {
    fn default() -> Self {
        Self::new()
    }
}
