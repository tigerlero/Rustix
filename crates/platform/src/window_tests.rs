//! Tests for window configuration and enums.

use crate::window::{WindowConfig, FullscreenMode, CursorMode};

#[test]
fn fullscreen_mode_default() {
    assert_eq!(FullscreenMode::default(), FullscreenMode::Windowed);
}

#[test]
fn fullscreen_mode_variants() {
    assert_ne!(FullscreenMode::Windowed, FullscreenMode::Exclusive);
    assert_ne!(FullscreenMode::Exclusive, FullscreenMode::Borderless);
}

#[test]
fn cursor_mode_default() {
    assert_eq!(CursorMode::default(), CursorMode::Normal);
}

#[test]
fn cursor_mode_variants() {
    assert_ne!(CursorMode::Normal, CursorMode::Hidden);
    assert_ne!(CursorMode::Hidden, CursorMode::Captured);
    assert_ne!(CursorMode::Captured, CursorMode::RawDelta);
}

#[test]
fn window_config_default() {
    let cfg = WindowConfig::default();
    assert_eq!(cfg.title, "Rustix Engine");
    assert_eq!(cfg.width, 1280);
    assert_eq!(cfg.height, 720);
    assert!(!cfg.fullscreen);
    assert_eq!(cfg.fullscreen_mode, FullscreenMode::Windowed);
    assert!(cfg.resizable);
    assert!(cfg.decorations);
    assert_eq!(cfg.cursor_mode, CursorMode::Normal);
}

#[test]
fn window_config_clone() {
    let cfg = WindowConfig::default();
    let cloned = cfg.clone();
    assert_eq!(cfg.title, cloned.title);
    assert_eq!(cfg.width, cloned.width);
}
