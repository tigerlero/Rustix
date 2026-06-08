//! Tests for animation keyframes, tracks, clips, and animators.

use rustix_core::math::{Vec3, Quat};
use crate::*;

#[test]
fn animation_track_empty() {
    let track = AnimationTrack::new();
    assert!(track.sample(0.0).is_none());
}

#[test]
fn animation_track_single_keyframe() {
    let mut track = AnimationTrack::new();
    track.keyframes.push(Keyframe { time: 0.0, value: Vec3::X });
    assert_eq!(track.sample(0.0), Some(Vec3::X));
    assert_eq!(track.sample(5.0), Some(Vec3::X));
}

#[test]
fn animation_track_interpolation() {
    let mut track = AnimationTrack::new();
    track.keyframes.push(Keyframe { time: 0.0, value: Vec3::ZERO });
    track.keyframes.push(Keyframe { time: 1.0, value: Vec3::X });
    let v = track.sample(0.5).unwrap();
    assert!((v - Vec3::new(0.5, 0.0, 0.0)).length() < 1e-4);
}

#[test]
fn animation_track_clamped_before_start() {
    let mut track = AnimationTrack::new();
    track.keyframes.push(Keyframe { time: 1.0, value: Vec3::X });
    track.keyframes.push(Keyframe { time: 2.0, value: Vec3::Y });
    assert_eq!(track.sample(0.0), Some(Vec3::X));
}

#[test]
fn animation_track_clamped_after_end() {
    let mut track = AnimationTrack::new();
    track.keyframes.push(Keyframe { time: 0.0, value: Vec3::X });
    track.keyframes.push(Keyframe { time: 1.0, value: Vec3::Y });
    assert_eq!(track.sample(2.0), Some(Vec3::Y));
}

#[test]
fn rotation_track_empty() {
    let track = RotationTrack::new();
    assert!(track.sample(0.0).is_none());
}

#[test]
fn rotation_track_single_keyframe() {
    let mut track = RotationTrack::new();
    track.keyframes.push(QuatKeyframe { time: 0.0, value: Quat::IDENTITY });
    assert_eq!(track.sample(0.0), Some(Quat::IDENTITY));
}

#[test]
fn event_track_events_between_simple() {
    let mut track = EventTrack::new();
    track.events.push(AnimationEvent { time: 0.5, name: "footstep".to_string() });
    track.events.push(AnimationEvent { time: 1.0, name: "jump".to_string() });
    let evs = track.events_between(0.0, 1.0, 2.0, false);
    assert_eq!(evs.len(), 2);
}

#[test]
fn event_track_events_between_looped_wrap() {
    let mut track = EventTrack::new();
    track.events.push(AnimationEvent { time: 0.1, name: "start".to_string() });
    track.events.push(AnimationEvent { time: 1.9, name: "end".to_string() });
    let evs = track.events_between(1.8, 0.2, 2.0, true);
    assert_eq!(evs.len(), 2);
}

#[test]
fn animation_clip_sample() {
    let mut clip = AnimationClip::new("test", 1.0);
    clip.position_track.keyframes.push(Keyframe { time: 0.0, value: Vec3::ZERO });
    clip.position_track.keyframes.push(Keyframe { time: 1.0, value: Vec3::X });
    let (pos, rot, scl) = clip.sample(0.5);
    assert!(pos.is_some());
    assert!(rot.is_none());
    assert!(scl.is_none());
}

#[test]
fn animation_clip_sample_events() {
    let mut clip = AnimationClip::new("test", 2.0);
    clip.event_track.events.push(AnimationEvent { time: 0.5, name: "hit".to_string() });
    let evs = clip.sample_events(0.0, 1.0, false);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].name, "hit");
}

#[test]
fn animator_default() {
    let anim = Animator::default();
    assert_eq!(anim.time, 0.0);
    assert_eq!(anim.speed, 1.0);
    assert!(anim.playing);
    assert!(anim.looped);
}

#[test]
fn update_animators_advances_time() {
    let mut world = hecs::World::new();
    let e = world.spawn(());

    let mut animator = Animator::default();
    animator.clip_name = "run".to_string();
    animator.speed = 1.0;
    animator.looped = false;

    let mut clip = AnimationClip::new("run", 2.0);
    clip.position_track.keyframes.push(Keyframe { time: 0.0, value: Vec3::ZERO });
    clip.position_track.keyframes.push(Keyframe { time: 2.0, value: Vec3::X });

    let mut clips = std::collections::HashMap::new();
    clips.insert("run".to_string(), clip);

    let mut animators = vec![(e, &mut animator)];
    let results = update_animators(&mut animators, &clips, 1.0);

    assert_eq!(animator.time, 1.0);
    assert!(results[0].1.is_some());
}

#[test]
fn update_animators_stops_at_end() {
    let mut animator = Animator::default();
    animator.clip_name = "run".to_string();
    animator.looped = false;

    let mut clip = AnimationClip::new("run", 1.0);
    clip.position_track.keyframes.push(Keyframe { time: 0.0, value: Vec3::ZERO });

    let mut clips = std::collections::HashMap::new();
    clips.insert("run".to_string(), clip);

    let e = hecs::World::new().spawn(());
    let mut animators = vec![(e, &mut animator)];
    update_animators(&mut animators, &clips, 2.0);

    assert!(!animator.playing);
    assert_eq!(animator.time, 1.0);
}

#[test]
fn update_animators_loops() {
    let mut animator = Animator::default();
    animator.clip_name = "run".to_string();
    animator.looped = true;

    let mut clip = AnimationClip::new("run", 1.0);
    clip.position_track.keyframes.push(Keyframe { time: 0.0, value: Vec3::ZERO });

    let mut clips = std::collections::HashMap::new();
    clips.insert("run".to_string(), clip);

    let e = hecs::World::new().spawn(());
    let mut animators = vec![(e, &mut animator)];
    update_animators(&mut animators, &clips, 2.5);

    assert!(animator.playing);
    assert!((animator.time - 0.5).abs() < 1e-4);
}

#[test]
fn blend_animator_new() {
    let ba = BlendAnimator::new("idle");
    assert_eq!(ba.current.clip_name, "idle");
    assert_eq!(ba.blend_weight, 1.0);
}

#[test]
fn blend_animator_transition() {
    let mut ba = BlendAnimator::new("idle");
    ba.transition_to("run", 0.5);
    assert_eq!(ba.current.clip_name, "run");
    assert_eq!(ba.previous.clip_name, "idle");
    assert_eq!(ba.blend_weight, 0.0);
}

#[test]
fn blend_animator_update_blend() {
    let mut ba = BlendAnimator::new("idle");
    ba.transition_to("run", 1.0);

    let mut clip_a = AnimationClip::new("idle", 1.0);
    clip_a.position_track.keyframes.push(Keyframe { time: 0.0, value: Vec3::ZERO });

    let mut clip_b = AnimationClip::new("run", 1.0);
    clip_b.position_track.keyframes.push(Keyframe { time: 0.0, value: Vec3::X });

    let mut clips = std::collections::HashMap::new();
    clips.insert("idle".to_string(), clip_a);
    clips.insert("run".to_string(), clip_b);

    let result = ba.update(&clips, 0.5);
    assert!(result.0.is_some());
    assert!(ba.blend_weight > 0.0 && ba.blend_weight <= 1.0);
}

#[test]
fn root_motion_extracts_delta() {
    let mut clip = AnimationClip::new("run", 1.0);
    clip.position_track.keyframes.push(Keyframe { time: 0.0, value: Vec3::ZERO });
    clip.position_track.keyframes.push(Keyframe { time: 1.0, value: Vec3::X });
    clip.rotation_track.keyframes.push(QuatKeyframe { time: 0.0, value: Quat::IDENTITY });
    clip.rotation_track.keyframes.push(QuatKeyframe { time: 1.0, value: Quat::IDENTITY });

    let motion = clip.extract_root_motion(0.0, 1.0);
    assert_eq!(motion.delta_position, Vec3::X);
    assert_eq!(motion.delta_rotation, Quat::IDENTITY);
}
