//! Tests for Tracy GPU profiling stubs.

use crate::tracy_gpu::{TracyGpuZone, begin_zone, end_zone, collect_timestamps};

#[test]
fn tracy_gpu_zone_new() {
    let zone = TracyGpuZone::new("test_zone");
    assert_eq!(zone.name, "test_zone");
}

#[test]
fn tracy_gpu_begin_end_zone() {
    let zone = begin_zone("my_zone");
    assert_eq!(zone.name, "my_zone");
    end_zone(zone);
}

#[test]
fn tracy_gpu_collect_timestamps() {
    collect_timestamps();
}

#[test]
fn tracy_gpu_zone_debug() {
    let zone = TracyGpuZone::new("debug_zone");
    let s = format!("{:?}", zone);
    assert!(s.contains("debug_zone"));
}
