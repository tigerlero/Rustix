//! Tests for CCD IK solver.

use rustix_core::math::{Vec3, Quat};
use crate::ik::{CcdIkSolver, IkJoint};

fn straight_chain(n: usize, length: f32) -> Vec<IkJoint> {
    let mut chain = Vec::with_capacity(n);
    for i in 0..n {
        chain.push(IkJoint {
            position: Vec3::new(0.0, i as f32 * length, 0.0),
            rotation: Quat::IDENTITY,
            length,
        });
    }
    chain
}

#[test]
fn solver_default() {
    let solver = CcdIkSolver::default();
    assert_eq!(solver.max_iterations, 10);
    assert_eq!(solver.tolerance, 0.001);
}

#[test]
fn solver_new() {
    let solver = CcdIkSolver::new();
    assert_eq!(solver.max_iterations, 10);
}

#[test]
fn solve_empty_chain() {
    let solver = CcdIkSolver::new();
    let mut chain = vec![];
    assert!(!solver.solve(&mut chain, Vec3::Y));
}

#[test]
fn solve_single_joint_no_move() {
    let solver = CcdIkSolver::new();
    let mut chain = vec![IkJoint {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        length: 0.0,
    }];
    assert!(solver.solve(&mut chain, Vec3::ZERO));
}

#[test]
fn solve_already_at_target() {
    let solver = CcdIkSolver::new();
    let mut chain = straight_chain(2, 1.0);
    // End-effector is already at (0, 1, 0)
    assert!(solver.solve(&mut chain, Vec3::new(0.0, 1.0, 0.0)));
}

#[test]
fn solve_reaches_target() {
    let solver = CcdIkSolver::new();
    let mut chain = straight_chain(3, 1.0);
    // Target is reachable: (0, 2, 0) — end-effector of 3-segment straight chain
    assert!(solver.solve(&mut chain, Vec3::new(0.0, 2.0, 0.0)));
}

#[test]
fn solve_bends_toward_target() {
    let solver = CcdIkSolver::new();
    let mut chain = straight_chain(3, 1.0);
    // Target within reach: total chain length = 2 (joints 0 and 1 have length)
    let target = Vec3::new(1.0, 1.5, 0.0);
    let reached = solver.solve(&mut chain, target);
    let mut positions = vec![chain[0].position];
    for i in 0..chain.len().saturating_sub(1) {
        positions.push(positions[i] + chain[i].rotation * Vec3::Y * chain[i].length);
    }
    let end_effector = positions.last().copied().unwrap();
    let dist = end_effector.distance(target);
    assert!(dist < solver.tolerance || !reached && dist < 0.1,
            "end effector at {:?}, target at {:?}, dist={}", end_effector, target, dist);
}

#[test]
fn solve_chain_reaches_sideways() {
    let solver = CcdIkSolver::new();
    let mut chain = straight_chain(5, 1.0);
    let target = Vec3::new(3.0, 2.0, 0.0);
    let reached = solver.solve(&mut chain, target);
    let mut positions = vec![chain[0].position];
    for i in 0..chain.len().saturating_sub(1) {
        positions.push(positions[i] + chain[i].rotation * Vec3::Y * chain[i].length);
    }
    let end_effector = positions.last().copied().unwrap();
    let dist = end_effector.distance(target);
    // Should be very close even if not exact
    assert!(dist < 0.1, "end effector at {:?}, target at {:?}, dist={}", end_effector, target, dist);
}

#[test]
fn solve_tolerance_matters() {
    let solver = CcdIkSolver {
        max_iterations: 10,
        tolerance: 0.001,
    };
    let mut chain = straight_chain(2, 1.0);
    assert!(solver.solve(&mut chain, Vec3::new(0.0, 1.0, 0.0)));
}

#[test]
fn solve_multiple_iterations() {
    let solver = CcdIkSolver {
        max_iterations: 100,
        tolerance: 0.01,
    };
    let mut chain = straight_chain(5, 1.0);
    let target = Vec3::new(2.5, 3.0, 0.0);
    let reached = solver.solve(&mut chain, target);
    let mut positions = vec![chain[0].position];
    for i in 0..chain.len().saturating_sub(1) {
        positions.push(positions[i] + chain[i].rotation * Vec3::Y * chain[i].length);
    }
    let end_effector = positions.last().copied().unwrap();
    let dist = end_effector.distance(target);
    assert!(dist < solver.tolerance, "dist={}", dist);
    assert!(reached);
}

#[test]
fn ik_joint_debug_clone() {
    let joint = IkJoint {
        position: Vec3::ONE,
        rotation: Quat::IDENTITY,
        length: 1.0,
    };
    let cloned = joint.clone();
    assert_eq!(joint, cloned);
}
