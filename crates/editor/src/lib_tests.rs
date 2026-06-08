//! Tests for editor shared types and utility functions.

use rustix_core::math::Vec3;
use crate::{GizmoMode, GizmoAxis, SelectionState, point_line_distance, gizmo_screen_size};

#[test]
fn gizmo_mode_default() {
    assert_eq!(GizmoMode::default(), GizmoMode::Translate);
}

#[test]
fn gizmo_mode_variants() {
    assert_ne!(GizmoMode::Translate, GizmoMode::Rotate);
    assert_ne!(GizmoMode::Rotate, GizmoMode::Scale);
}

#[test]
fn gizmo_axis_variants() {
    assert_ne!(GizmoAxis::X, GizmoAxis::Y);
    assert_ne!(GizmoAxis::Y, GizmoAxis::Z);
    assert_ne!(GizmoAxis::XY, GizmoAxis::XZ);
}

#[test]
fn selection_state_default() {
    let state = SelectionState::default();
    assert!(state.selected.is_none());
    assert_eq!(state.gizmo_mode, GizmoMode::Translate);
    assert!(state.gizmo_active.is_none());
}

#[test]
fn point_line_distance_on_line() {
    let p = Vec3::new(0.5, 0.0, 0.0);
    let start = Vec3::ZERO;
    let end = Vec3::X;
    let dist = point_line_distance(p, start, end);
    assert!((dist - 0.0).abs() < 1e-4);
}

#[test]
fn point_line_distance_off_line() {
    let p = Vec3::new(0.5, 1.0, 0.0);
    let start = Vec3::ZERO;
    let end = Vec3::X;
    let dist = point_line_distance(p, start, end);
    assert!((dist - 1.0).abs() < 1e-4);
}

#[test]
fn point_line_distance_degenerate_line() {
    let p = Vec3::new(1.0, 0.0, 0.0);
    let start = Vec3::ZERO;
    let end = Vec3::ZERO;
    let dist = point_line_distance(p, start, end);
    assert!((dist - 1.0).abs() < 1e-4);
}

#[test]
fn gizmo_screen_size_nonzero() {
    let world_pos = Vec3::new(1.0, 1.0, 1.0);
    let view_proj = rustix_core::math::Mat4::IDENTITY;
    let size = gizmo_screen_size(world_pos, view_proj, 1080.0);
    assert!(size >= 0.0);
}
