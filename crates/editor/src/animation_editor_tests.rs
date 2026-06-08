//! Tests for animation editor timeline and keyframes.

use rustix_core::math::{Vec3, Quat};
use crate::animation_editor::*;

#[test]
fn keyframe_value_variants() {
    let v1 = KeyframeValue::Float(1.0);
    let v2 = KeyframeValue::Vec3(Vec3::X);
    assert_ne!(std::mem::discriminant(&v1), std::mem::discriminant(&v2));
}

#[test]
fn keyframe_value_equality() {
    assert_eq!(KeyframeValue::Float(1.0), KeyframeValue::Float(1.0));
    assert_ne!(KeyframeValue::Float(1.0), KeyframeValue::Float(2.0));
}

#[test]
fn interpolation_type_variants() {
    assert_ne!(InterpolationType::Step, InterpolationType::Linear);
    assert_ne!(InterpolationType::Linear, InterpolationType::Smooth);
}

#[test]
fn animation_track_new() {
    let track = AnimationTrack::new("position");
    assert_eq!(track.name, "position");
    assert!(track.keyframes.is_empty());
}

#[test]
fn animation_track_add_keyframe() {
    let mut track = AnimationTrack::new("rotation");
    track.add_keyframe(Keyframe { time: 0.5, value: KeyframeValue::Float(1.0), interpolation: InterpolationType::Linear });
    track.add_keyframe(Keyframe { time: 0.2, value: KeyframeValue::Float(0.0), interpolation: InterpolationType::Linear });
    assert_eq!(track.keyframes.len(), 2);
    assert_eq!(track.keyframes[0].time, 0.2); // sorted
    assert_eq!(track.keyframes[1].time, 0.5);
}

#[test]
fn animation_track_remove_keyframe_at() {
    let mut track = AnimationTrack::new("scale");
    track.add_keyframe(Keyframe { time: 0.0, value: KeyframeValue::Float(1.0), interpolation: InterpolationType::Linear });
    track.add_keyframe(Keyframe { time: 1.0, value: KeyframeValue::Float(2.0), interpolation: InterpolationType::Linear });
    track.remove_keyframe_at(1.0, 0.01);
    assert_eq!(track.keyframes.len(), 1);
    assert_eq!(track.keyframes[0].time, 0.0);
}

#[test]
fn timeline_state_default() {
    let ts = TimelineState::default();
    assert_eq!(ts.current_time, 0.0);
    assert_eq!(ts.duration, 1.0);
    assert!(!ts.playing);
    assert!(ts.loop_playback);
    assert_eq!(ts.playback_speed, 1.0);
    assert!(ts.tracks.is_empty());
}

#[test]
fn timeline_state_new() {
    let ts = TimelineState::new();
    assert_eq!(ts.current_time, 0.0);
}

#[test]
fn timeline_update_advances_time() {
    let mut ts = TimelineState::default();
    ts.play();
    ts.update(0.5);
    assert_eq!(ts.current_time, 0.5);
    assert!(ts.playing);
}

#[test]
fn timeline_update_loops() {
    let mut ts = TimelineState::default();
    ts.duration = 1.0;
    ts.current_time = 0.9;
    ts.play();
    ts.update(0.2);
    assert!(ts.current_time < 0.5); // wrapped around
    assert!(ts.playing);
}

#[test]
fn timeline_update_no_loop_stops() {
    let mut ts = TimelineState::default();
    ts.loop_playback = false;
    ts.duration = 1.0;
    ts.current_time = 0.9;
    ts.play();
    ts.update(0.2);
    assert_eq!(ts.current_time, 1.0);
    assert!(!ts.playing);
}

#[test]
fn timeline_update_when_paused() {
    let mut ts = TimelineState::default();
    ts.playing = false;
    ts.update(0.5);
    assert_eq!(ts.current_time, 0.0);
}

#[test]
fn timeline_seek_clamps() {
    let mut ts = TimelineState::default();
    ts.duration = 1.0;
    ts.seek(-5.0);
    assert_eq!(ts.current_time, 0.0);
    ts.seek(5.0);
    assert_eq!(ts.current_time, 1.0);
}

#[test]
fn timeline_pause() {
    let mut ts = TimelineState::default();
    ts.play();
    ts.pause();
    assert!(!ts.playing);
}

#[test]
fn timeline_stop_resets() {
    let mut ts = TimelineState::default();
    ts.play();
    ts.update(0.5);
    ts.stop();
    assert!(!ts.playing);
    assert_eq!(ts.current_time, 0.0);
}
