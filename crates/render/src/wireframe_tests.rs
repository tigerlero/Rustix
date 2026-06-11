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
    assert_ne!(DebugOverlay::Overdraw, DebugOverlay::LightComplexity);
}

#[test]
fn debug_render_mode_default() {
    let mode = DebugRenderMode::default();
    assert_eq!(mode.overlay, DebugOverlay::None);
    assert_eq!(mode.wireframe, WireframeMode::Off);
    assert!(!mode.is_wireframe());
    assert!(!mode.is_overdraw());
    assert!(!mode.is_light_complexity());
}

#[test]
fn debug_render_mode_wireframe() {
    let mode = DebugRenderMode { overlay: DebugOverlay::None, wireframe: WireframeMode::On };
    assert!(mode.is_wireframe());
}

#[test]
fn debug_render_mode_overdraw() {
    let mode = DebugRenderMode { overlay: DebugOverlay::Overdraw, wireframe: WireframeMode::Off };
    assert!(mode.is_overdraw());
}

#[test]
fn debug_render_mode_light_complexity() {
    let mode = DebugRenderMode { overlay: DebugOverlay::LightComplexity, wireframe: WireframeMode::Off };
    assert!(mode.is_light_complexity());
}
