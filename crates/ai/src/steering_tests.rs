//! Tests for steering behaviors.

use rustix_core::math::Vec3;
use crate::steering::{Agent, seek, flee, arrive, wander, avoid_obstacles, separation, alignment, cohesion, combine, integrate};

#[test]
fn seek_returns_force_toward_target() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let force = seek(&agent, Vec3::new(10.0, 0.0, 0.0));
    assert!(force.x > 0.0, "seek should point toward target");
    assert_eq!(force.y, 0.0);
    assert_eq!(force.z, 0.0);
}

#[test]
fn seek_zero_when_at_target() {
    let agent = Agent::new(Vec3::new(5.0, 0.0, 0.0), 10.0, 5.0);
    let force = seek(&agent, Vec3::new(5.0, 0.0, 0.0));
    // desired is zero vector, normalize_or_zero gives zero
    assert_eq!(force, Vec3::ZERO);
}

#[test]
fn flee_returns_force_away_from_target() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let force = flee(&agent, Vec3::new(10.0, 0.0, 0.0));
    assert!(force.x < 0.0, "flee should point away from target");
}

#[test]
fn arrive_slows_near_target() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    // Far away: should want full speed
    let far = arrive(&agent, Vec3::new(100.0, 0.0, 0.0), 10.0);
    let near = arrive(&agent, Vec3::new(5.0, 0.0, 0.0), 10.0);
    // Near target should produce smaller desired speed
    assert!(far.length() > near.length(), "arrive should reduce speed when close");
}

#[test]
fn arrive_zero_when_very_close() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let force = arrive(&agent, Vec3::new(0.0001, 0.0, 0.0), 1.0);
    assert_eq!(force, Vec3::ZERO);
}

#[test]
fn wander_returns_nonzero_force() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let mut angle = 0.0f32;
    let force = wander(&agent, 5.0, 2.0, &mut angle, 0.5, 1.0);
    assert!(force.length() > 0.0, "wander should produce a steering force");
}

#[test]
fn wander_angle_changes() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let mut angle = 0.0f32;
    wander(&agent, 5.0, 2.0, &mut angle, 0.5, 1.0);
    assert_ne!(angle, 0.0, "wander should mutate the angle");
}

#[test]
fn avoid_obstacles_zero_when_clear() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let obstacles = [(Vec3::new(100.0, 0.0, 0.0), 5.0f32)];
    let force = avoid_obstacles(&agent, &obstacles, 10.0);
    assert_eq!(force, Vec3::ZERO, "no obstacles in path");
}

#[test]
fn avoid_obstacles_pushes_away() {
    let mut agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    agent.velocity = Vec3::new(10.0, 0.0, 0.0); // moving along +X
    // Obstacle directly ahead on X axis
    let obstacles = [(Vec3::new(5.0, 0.0, 0.0), 2.0f32)];
    let force = avoid_obstacles(&agent, &obstacles, 10.0);
    assert!(force.length() > 0.0, "should steer away from obstacle");
}

#[test]
fn separation_zero_when_alone() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let force = separation(&agent, &[], 5.0);
    assert_eq!(force, Vec3::ZERO);
}

#[test]
fn separation_pushes_from_neighbors() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let neighbors = [Vec3::new(2.0, 0.0, 0.0)];
    let force = separation(&agent, &neighbors, 5.0);
    assert!(force.x < 0.0, "should push away from neighbor on +X");
}

#[test]
fn separation_ignores_far_neighbors() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let neighbors = [Vec3::new(100.0, 0.0, 0.0)];
    let force = separation(&agent, &neighbors, 5.0);
    assert_eq!(force, Vec3::ZERO, "far neighbor should be ignored");
}

#[test]
fn alignment_zero_when_alone() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let force = alignment(&agent, &[]);
    assert_eq!(force, Vec3::ZERO);
}

#[test]
fn alignment_matches_neighbor_velocity() {
    let mut agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    agent.velocity = Vec3::new(1.0, 0.0, 0.0);
    let neighbors = [Vec3::new(5.0, 0.0, 0.0)]; // positions irrelevant for alignment
    let force = alignment(&agent, &[Vec3::new(0.0, 5.0, 0.0)]);
    // Average velocity is (0,5,0), agent is (1,0,0), so force should push toward +Y
    assert!(force.y > 0.0, "should align toward neighbor velocity direction");
}

#[test]
fn cohesion_zero_when_alone() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let force = cohesion(&agent, &[]);
    assert_eq!(force, Vec3::ZERO);
}

#[test]
fn cohesion_seeks_center_of_neighbors() {
    let agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let neighbors = [Vec3::new(10.0, 0.0, 0.0), Vec3::new(20.0, 0.0, 0.0)];
    let force = cohesion(&agent, &neighbors);
    // Center is at (15,0,0), so seek should push +X
    assert!(force.x > 0.0, "should seek toward average neighbor position");
}

#[test]
fn combine_weights_forces() {
    let a = Vec3::new(1.0, 0.0, 0.0);
    let b = Vec3::new(0.0, 2.0, 0.0);
    let result = combine(&[(a, 2.0), (b, 3.0)]);
    assert_eq!(result, Vec3::new(2.0, 6.0, 0.0));
}

#[test]
fn combine_empty_is_zero() {
    let result = combine(&[]);
    assert_eq!(result, Vec3::ZERO);
}

#[test]
fn integrate_moves_agent() {
    let mut agent = Agent::new(Vec3::ZERO, 10.0, 5.0);
    let steering = Vec3::new(1.0, 0.0, 0.0);
    integrate(&mut agent, steering, 1.0);
    assert!(agent.position.x > 0.0, "agent should move in steering direction");
    assert!(agent.velocity.length() > 0.0, "agent should have velocity");
}

#[test]
fn agent_apply_force_respects_max_force() {
    let mut agent = Agent::new(Vec3::ZERO, 10.0, 1.0);
    agent.apply_force(Vec3::new(100.0, 0.0, 0.0));
    assert!(agent.velocity.length() <= 1.0 + 1e-4, "velocity should be clamped by max_speed, but force is clamped by max_force");
}

#[test]
fn agent_apply_force_respects_max_speed() {
    let mut agent = Agent::new(Vec3::ZERO, 5.0, 100.0);
    agent.apply_force(Vec3::new(100.0, 0.0, 0.0));
    assert!(agent.velocity.length() <= 5.0 + 1e-4, "velocity should be clamped by max_speed");
}
