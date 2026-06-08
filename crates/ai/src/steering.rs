//! Steering behaviors for autonomous agents.
//!
//! Classic Reynolds-style steering forces that can be combined to
//! produce emergent locomotion: seek, flee, arrive, wander,
//! obstacle avoidance, and separation.

use rustix_core::math::Vec3;

/// Agent state used as input to steering behaviors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Agent {
    pub position: Vec3,
    pub velocity: Vec3,
    pub max_speed: f32,
    pub max_force: f32,
}

impl Agent {
    pub fn new(position: Vec3, max_speed: f32, max_force: f32) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            max_speed,
            max_force,
        }
    }

    /// Apply a steering force to this agent, respecting `max_force`.
    pub fn apply_force(&mut self, force: Vec3) {
        let clamped = force.clamp_length_max(self.max_force);
        self.velocity += clamped;
        self.velocity = self.velocity.clamp_length_max(self.max_speed);
    }
}

/// Seek the target position at maximum speed.
pub fn seek(agent: &Agent, target: Vec3) -> Vec3 {
    let desired = target - agent.position;
    let desired_velocity = desired.normalize_or_zero() * agent.max_speed;
    desired_velocity - agent.velocity
}

/// Flee from the target position.
pub fn flee(agent: &Agent, target: Vec3) -> Vec3 {
    let desired = agent.position - target;
    let desired_velocity = desired.normalize_or_zero() * agent.max_speed;
    desired_velocity - agent.velocity
}

/// Arrive at the target, slowing down within `slowing_distance`.
pub fn arrive(agent: &Agent, target: Vec3, slowing_distance: f32) -> Vec3 {
    let to_target = target - agent.position;
    let distance = to_target.length();
    if distance < 0.001 {
        return Vec3::ZERO;
    }
    let desired_speed = if distance < slowing_distance {
        agent.max_speed * (distance / slowing_distance)
    } else {
        agent.max_speed
    };
    let desired_velocity = to_target.normalize() * desired_speed;
    desired_velocity - agent.velocity
}

/// Wander using a random displacement each frame.
///
/// `circle_distance` — how far ahead the wander circle is.
/// `circle_radius` — radius of the wander circle.
/// `wander_angle` — current wander angle (radians), mutated in place.
/// `angle_change` — max random change per frame (radians).
/// `random_signed` — a value in [-1, 1] from the caller's RNG.
pub fn wander(
    agent: &Agent,
    circle_distance: f32,
    circle_radius: f32,
    wander_angle: &mut f32,
    angle_change: f32,
    random_signed: f32,
) -> Vec3 {
    let displacement = agent.velocity.normalize_or_zero() * circle_distance;
    *wander_angle += random_signed * angle_change;
    let circle_offset = Vec3::new(
        circle_radius * wander_angle.cos(),
        0.0,
        circle_radius * wander_angle.sin(),
    );
    let desired = displacement + circle_offset;
    desired.normalize_or_zero() * agent.max_speed - agent.velocity
}

/// Avoid nearby obstacles using a feeler ray ahead of the agent.
///
/// `obstacles` — list of obstacle center positions and radii.
/// `feeler_length` — how far ahead to cast the avoidance ray.
pub fn avoid_obstacles(
    agent: &Agent,
    obstacles: &[(Vec3, f32)],
    feeler_length: f32,
) -> Vec3 {
    let ahead = agent.position + agent.velocity.normalize_or_zero() * feeler_length;
    let ahead2 = agent.position + agent.velocity.normalize_or_zero() * (feeler_length * 0.5);

    let mut closest = None;
    for &(pos, radius) in obstacles {
        let d1 = (pos - ahead).length();
        let d2 = (pos - ahead2).length();
        let d3 = (pos - agent.position).length();
        if d1 < radius || d2 < radius || d3 < radius {
            let dist = d1.min(d2).min(d3);
            if closest.map_or(true, |(_, d, _)| dist < d) {
                closest = Some((pos, dist, radius));
            }
        }
    }

    if let Some((obstacle_pos, _dist, _radius)) = closest {
        let avoidance = ahead - obstacle_pos;
        avoidance.normalize_or_zero() * agent.max_force
    } else {
        Vec3::ZERO
    }
}

/// Keep a minimum distance from neighboring agents.
pub fn separation(agent: &Agent, neighbors: &[Vec3], desired_separation: f32) -> Vec3 {
    let mut sum = Vec3::ZERO;
    let mut count = 0u32;
    for &other in neighbors {
        let diff = agent.position - other;
        let dist = diff.length();
        if dist > 0.001 && dist < desired_separation {
            sum += diff.normalize() / dist;
            count += 1;
        }
    }
    if count == 0 {
        return Vec3::ZERO;
    }
    let avg = sum / count as f32;
    let desired = avg.normalize_or_zero() * agent.max_speed;
    desired - agent.velocity
}

/// Align velocity with the average velocity of neighbors.
pub fn alignment(agent: &Agent, neighbor_velocities: &[Vec3]) -> Vec3 {
    if neighbor_velocities.is_empty() {
        return Vec3::ZERO;
    }
    let avg: Vec3 = neighbor_velocities.iter().copied().sum::<Vec3>() / neighbor_velocities.len() as f32;
    let desired = avg.normalize_or_zero() * agent.max_speed;
    desired - agent.velocity
}

/// Steer toward the average position of neighbors (cohesion).
pub fn cohesion(agent: &Agent, neighbors: &[Vec3]) -> Vec3 {
    if neighbors.is_empty() {
        return Vec3::ZERO;
    }
    let avg_pos: Vec3 = neighbors.iter().copied().sum::<Vec3>() / neighbors.len() as f32;
    seek(agent, avg_pos)
}

/// Combine multiple steering forces with per-behavior weights.
pub fn combine(forces: &[(Vec3, f32)]) -> Vec3 {
    forces.iter().map(|(f, w)| *f * *w).sum::<Vec3>()
}

/// Update an agent's position using Euler integration with a given
/// steering force.
pub fn integrate(agent: &mut Agent, steering: Vec3, dt: f32) {
    agent.apply_force(steering);
    agent.position += agent.velocity * dt;
}
