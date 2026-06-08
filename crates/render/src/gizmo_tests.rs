//! Tests for gizmo line generation.

use glam::Vec3;
use crate::gizmo::{wireframe_box, generate_audio_gizmo, flatten_gizmo_lines, AudioGizmo};

#[test]
fn test_wireframe_box() {
    let lines = wireframe_box(Vec3::ZERO, Vec3::ONE, [1.0; 4]);
    assert_eq!(lines.len(), 12);
}

#[test]
fn test_audio_gizmo() {
    let gizmo = AudioGizmo {
        position: Vec3::new(1.0, 2.0, 3.0),
        min_distance: 1.0,
        max_distance: 5.0,
        direction: Some(Vec3::Y),
        ..Default::default()
    };
    let lines = generate_audio_gizmo(&gizmo);
    assert!(!lines.is_empty());
}

#[test]
fn test_flatten() {
    let lines = wireframe_box(Vec3::ZERO, Vec3::ONE, [1.0; 4]);
    let flat = flatten_gizmo_lines(&lines);
    assert_eq!(flat.len(), lines.len() * 14);
}
