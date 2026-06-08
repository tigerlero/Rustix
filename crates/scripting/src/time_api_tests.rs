//! Tests for script time API.

use crate::time_api::ScriptTime;

#[test]
fn new_time_is_zero() {
    let t = ScriptTime::new();
    assert_eq!(t.delta_time, 0.0);
    assert_eq!(t.elapsed, 0.0);
    assert_eq!(t.frame_count, 0);
}

#[test]
fn tick_increments_elapsed() {
    let mut t = ScriptTime::new();
    t.tick(0.016);
    assert!((t.elapsed - 0.016).abs() < 1e-6);
    assert_eq!(t.frame_count, 1);
}

#[test]
fn tick_accumulates() {
    let mut t = ScriptTime::new();
    t.tick(0.016);
    t.tick(0.016);
    t.tick(0.016);
    assert!((t.elapsed - 0.048).abs() < 1e-6);
    assert_eq!(t.frame_count, 3);
}

#[test]
fn tick_updates_delta_time() {
    let mut t = ScriptTime::new();
    t.tick(0.033);
    assert!((t.delta_time - 0.033).abs() < 1e-6);
    t.tick(0.016);
    assert!((t.delta_time - 0.016).abs() < 1e-6);
}
