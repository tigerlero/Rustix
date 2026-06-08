//! Tests for wireframe and debug overlay modes.

use crate::wireframe::*;

#[test]
fn wireframe_mode_default() {
    assert_eq!(WireframeMode::default(), WireframeMode::Off);
}

#[test]
fn wireframe_mode_variants() {
    assert_ne!(WireframeMode::Off, WireframeMode::On);
}

#[test]
fn debug_overlay_default() {
    assert_eq!(DebugOverlay::default(), DebugOverlay::None);
}

#[test]
fn debug_overlay_variants() {
    assert_ne!(DebugOverlay::None, DebugOverlay::Wireframe);
    assert_ne!(DebugOverlay::Wireframe, DebugOverlay::Normals);
    assert_ne!(DebugOverlay::Normals, DebugOverlay::TangentSpace);
    assert_ne!(DebugOverlay::TangentSpace, DebugOverlay::UV);
    assert_ne!(DebugOverlay::UV, DebugOverlay::Overdraw);
}
