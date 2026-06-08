//! Tests for platform error types and re-exports.

use crate::PlatformError;

#[test]
fn platform_error_window_creation_display() {
    let err = PlatformError::WindowCreation("failed".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("window creation failed"));
    assert!(msg.contains("failed"));
}

#[test]
fn platform_error_input_init_display() {
    let err = PlatformError::InputInit("bad".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("input initialization failed"));
}

#[test]
fn platform_error_surface_creation_display() {
    let err = PlatformError::SurfaceCreation("oops".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("surface creation failed"));
}

#[test]
fn platform_error_debug() {
    let err = PlatformError::WindowCreation("test".to_string());
    let dbg = format!("{:?}", err);
    assert!(dbg.contains("WindowCreation"));
}
