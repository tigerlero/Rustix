//! Tests for editor camera.

use rustix_core::math::Vec3;
use crate::camera::{CameraMode, EditorCamera};

#[test]
fn camera_mode_variants() {
    assert_ne!(CameraMode::Orbit, CameraMode::Fly);
    assert_ne!(CameraMode::Fly, CameraMode::Fps);
}

#[test]
fn editor_camera_default() {
    let cam = EditorCamera::default();
    assert_eq!(cam.mode, CameraMode::Orbit);
    assert_eq!(cam.fov_deg, 60.0);
    assert_eq!(cam.near, 0.1);
    assert_eq!(cam.far, 1000.0);
    assert_eq!(cam.distance, 20.0);
}

#[test]
fn editor_camera_new() {
    let cam = EditorCamera::new();
    assert_eq!(cam.mode, CameraMode::Orbit);
}

#[test]
fn editor_camera_mode_builder() {
    let cam = EditorCamera::new().mode(CameraMode::Fly);
    assert_eq!(cam.mode, CameraMode::Fly);
}

#[test]
fn editor_camera_orbit_drag() {
    let mut cam = EditorCamera::default();
    let yaw_before = cam.yaw;
    let pitch_before = cam.pitch;
    cam.orbit_drag(10.0, 5.0);
    assert_ne!(cam.yaw, yaw_before);
    assert_ne!(cam.pitch, pitch_before);
}

#[test]
fn editor_camera_orbit_zoom_clamps() {
    let mut cam = EditorCamera::default();
    cam.orbit_zoom(10000.0);
    assert_eq!(cam.distance, 0.5); // positive delta zooms in, clamped to min

    cam.orbit_zoom(-10000.0);
    assert_eq!(cam.distance, 500.0); // negative delta zooms out, clamped to max
}

#[test]
fn editor_camera_orbit_pan() {
    let mut cam = EditorCamera::default();
    let target_before = cam.target;
    cam.orbit_pan(10.0, 10.0);
    assert_ne!(cam.target, target_before);
}

#[test]
fn editor_camera_fly_move() {
    let mut cam = EditorCamera::new().mode(CameraMode::Fly);
    let pos_before = cam.position;
    cam.fly_move(1.0, 0.0, 0.0, 1.0);
    assert_ne!(cam.position, pos_before);
}

#[test]
fn editor_camera_fly_look() {
    let mut cam = EditorCamera::new().mode(CameraMode::Fly);
    let yaw_before = cam.yaw;
    let pitch_before = cam.pitch;
    cam.fly_look(10.0, 5.0);
    assert_ne!(cam.yaw, yaw_before);
    assert_ne!(cam.pitch, pitch_before);
}

#[test]
fn editor_camera_forward_not_zero() {
    let cam = EditorCamera::default();
    let fwd = cam.forward();
    assert!(fwd.length() > 0.9);
}

#[test]
fn editor_camera_right_orthogonal() {
    let cam = EditorCamera::default();
    let fwd = cam.forward();
    let right = cam.right();
    assert!(right.length() > 0.9);
    // Right should be roughly perpendicular to forward
    assert!((fwd.dot(right)).abs() < 0.01);
}

#[test]
fn editor_camera_up_orthogonal() {
    let cam = EditorCamera::default();
    let fwd = cam.forward();
    let up = cam.up();
    assert!(up.length() > 0.9);
    assert!((fwd.dot(up)).abs() < 0.01);
}

#[test]
fn editor_camera_view_matrix() {
    let cam = EditorCamera::default();
    let view = cam.view_matrix();
    // A view matrix should be invertible (det != 0)
    let det = view.determinant();
    assert!(det.abs() > 0.0001);
}

#[test]
fn editor_camera_projection_matrix() {
    let cam = EditorCamera::default();
    let proj = cam.projection_matrix(16.0 / 9.0);
    // Projection matrix should also be valid
    let det = proj.determinant();
    assert!(det.abs() > 0.0001);
}
