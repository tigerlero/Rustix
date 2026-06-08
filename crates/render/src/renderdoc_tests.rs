//! Tests for RenderDoc capture trigger.

use crate::renderdoc::RenderDocCapture;

#[test]
fn renderdoc_capture_new_default() {
    let cap = RenderDocCapture::new();
    assert!(!cap.enabled.load(std::sync::atomic::Ordering::Relaxed));
    assert!(!cap.capture_next_frame.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn renderdoc_capture_set_enabled() {
    let cap = RenderDocCapture::new();
    cap.set_enabled(true);
    assert!(cap.enabled.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn renderdoc_capture_trigger_and_consume() {
    let cap = RenderDocCapture::new();
    cap.set_enabled(true);
    cap.trigger();
    assert!(cap.capture_next_frame.load(std::sync::atomic::Ordering::Relaxed));
    assert!(cap.consume_trigger());
    assert!(!cap.capture_next_frame.load(std::sync::atomic::Ordering::Relaxed));
    assert!(!cap.consume_trigger());
}

#[test]
fn renderdoc_capture_trigger_ignored_when_disabled() {
    let cap = RenderDocCapture::new();
    cap.trigger();
    assert!(!cap.capture_next_frame.load(std::sync::atomic::Ordering::Relaxed));
}
