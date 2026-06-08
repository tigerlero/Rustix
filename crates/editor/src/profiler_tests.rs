//! Tests for profiler state and frame timing.

use crate::profiler::ProfilerState;

#[test]
fn profiler_state_new() {
    let ps = ProfilerState::new(60);
    assert_eq!(ps.max_samples, 60);
    assert!(ps.frame_times.is_empty());
    assert!(ps.current_frame.is_empty());
}

#[test]
fn profiler_begin_frame_clears_samples() {
    let mut ps = ProfilerState::new(60);
    ps.add_sample("render", 5.0);
    ps.begin_frame();
    assert!(ps.current_frame.is_empty());
}

#[test]
fn profiler_add_sample() {
    let mut ps = ProfilerState::new(60);
    ps.add_sample("render", 5.0);
    assert_eq!(ps.current_frame.len(), 1);
    assert_eq!(ps.current_frame[0].name, "render");
    assert_eq!(ps.current_frame[0].duration_ms, 5.0);
}

#[test]
fn profiler_end_frame_adds_total() {
    let mut ps = ProfilerState::new(60);
    ps.end_frame(16.0);
    assert_eq!(ps.frame_times.len(), 1);
    assert_eq!(ps.frame_times[0], 16.0);
}

#[test]
fn profiler_end_frame_max_samples_eviction() {
    let mut ps = ProfilerState::new(2);
    ps.end_frame(10.0);
    ps.end_frame(20.0);
    ps.end_frame(30.0);
    assert_eq!(ps.frame_times.len(), 2);
    assert_eq!(ps.frame_times[0], 20.0);
    assert_eq!(ps.frame_times[1], 30.0);
}

#[test]
fn profiler_average_fps() {
    let mut ps = ProfilerState::new(60);
    assert_eq!(ps.average_fps(), 0.0);

    ps.end_frame(16.0); // ~62.5 fps
    ps.end_frame(20.0); // ~50 fps
    let avg_fps = ps.average_fps();
    assert!(avg_fps > 50.0 && avg_fps < 70.0);
}

#[test]
fn profiler_frame_time_min_max() {
    let mut ps = ProfilerState::new(60);
    assert_eq!(ps.frame_time_min(), f32::INFINITY);
    assert_eq!(ps.frame_time_max(), 0.0);

    ps.end_frame(10.0);
    ps.end_frame(20.0);
    ps.end_frame(15.0);
    assert_eq!(ps.frame_time_min(), 10.0);
    assert_eq!(ps.frame_time_max(), 20.0);
}
