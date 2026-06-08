//! Tests for system monitor and thread recommendation.

use crate::system_monitor::{SystemMonitor, recommended_threads};

#[test]
fn monitor_first_call_returns_zero() {
    let mut m = SystemMonitor::new();
    assert_eq!(m.cpu_usage(), 0.0);
}

#[test]
fn recommended_at_zero_load() {
    assert_eq!(recommended_threads(8, 0.0, 2, 8), 8);
}

#[test]
fn recommended_at_full_load() {
    assert_eq!(recommended_threads(8, 1.0, 2, 8), 2);
}

#[test]
fn recommended_at_half_load() {
    assert_eq!(recommended_threads(8, 0.5, 2, 8), 5);
}

#[test]
fn recommended_respects_bounds() {
    assert_eq!(recommended_threads(8, -0.5, 2, 8), 8);
    assert_eq!(recommended_threads(8, 1.5, 2, 8), 2);
}

#[test]
fn recommended_min_equals_max() {
    assert_eq!(recommended_threads(4, 0.5, 4, 4), 4);
}
