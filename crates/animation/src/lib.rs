use rustix_core::math::{Vec3, Quat};
use std::collections::HashMap;

pub mod skeleton;
pub use skeleton::{Bone, Skeleton};

pub mod state_machine;
pub use state_machine::{AnimationState, AnimationStateMachine, Transition, TransitionCondition};

pub mod ik;
pub use ik::{CcdIkSolver, IkJoint};

#[cfg(test)]
pub mod lib_tests;

/// A single keyframe with a time and a Vec3 value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe {
    pub time: f32,
    pub value: Vec3,
}

/// A track of keyframes for one animated property.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationTrack {
    pub keyframes: Vec<Keyframe>,
}

impl AnimationTrack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sample the track at the given time using linear interpolation.
    pub fn sample(&self, time: f32) -> Option<Vec3> {
        if self.keyframes.is_empty() {
            return None;
        }
        if self.keyframes.len() == 1 {
            return Some(self.keyframes[0].value);
        }

        if time <= self.keyframes[0].time {
            return Some(self.keyframes[0].value);
        }
        let last = self.keyframes.last().unwrap();
        if time >= last.time {
            return Some(last.value);
        }

        for i in 0..self.keyframes.len() - 1 {
            let prev = &self.keyframes[i];
            let next = &self.keyframes[i + 1];
            if time >= prev.time && time <= next.time {
                let t = (time - prev.time) / (next.time - prev.time);
                return Some(prev.value.lerp(next.value, t));
            }
        }
        None
    }
}

/// A single rotation keyframe with a time and a Quat value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuatKeyframe {
    pub time: f32,
    pub value: Quat,
}

/// A rotation track using quaternion keyframes with SLERP interpolation.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RotationTrack {
    pub keyframes: Vec<QuatKeyframe>,
}

impl RotationTrack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sample the track at the given time using spherical linear interpolation.
    pub fn sample(&self, time: f32) -> Option<Quat> {
        if self.keyframes.is_empty() {
            return None;
        }
        if self.keyframes.len() == 1 {
            return Some(self.keyframes[0].value);
        }

        if time <= self.keyframes[0].time {
            return Some(self.keyframes[0].value);
        }
        let last = self.keyframes.last().unwrap();
        if time >= last.time {
            return Some(last.value);
        }

        for i in 0..self.keyframes.len() - 1 {
            let prev = &self.keyframes[i];
            let next = &self.keyframes[i + 1];
            if time >= prev.time && time <= next.time {
                let t = (time - prev.time) / (next.time - prev.time);
                return Some(prev.value.slerp(next.value, t));
            }
        }
        None
    }
}

/// An animation event fired at a specific time during playback.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationEvent {
    pub time: f32,
    pub name: String,
}

/// A track of named events at specific times.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventTrack {
    pub events: Vec<AnimationEvent>,
}

impl EventTrack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drain all events between `prev_time` and `current_time` (inclusive on start, exclusive on end).
    ///
    /// Handles looped playback by checking against clip `duration`.
    pub fn events_between(&self, prev_time: f32, current_time: f32, duration: f32, looped: bool) -> Vec<&AnimationEvent> {
        let mut result = Vec::new();
        if looped && current_time < prev_time {
            // Wrapped around: check [prev_time, duration] and [0, current_time]
            for ev in &self.events {
                if ev.time >= prev_time && ev.time <= duration {
                    result.push(ev);
                }
                if ev.time >= 0.0 && ev.time <= current_time {
                    result.push(ev);
                }
            }
        } else {
            for ev in &self.events {
                if ev.time >= prev_time && ev.time <= current_time {
                    result.push(ev);
                }
            }
        }
        result
    }
}

/// A named animation clip composed of position, rotation, and scale tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    pub position_track: AnimationTrack,
    pub rotation_track: RotationTrack,
    pub scale_track: AnimationTrack,
    pub event_track: EventTrack,
}

impl AnimationClip {
    pub fn new(name: impl Into<String>, duration: f32) -> Self {
        Self {
            name: name.into(),
            duration,
            position_track: AnimationTrack::new(),
            rotation_track: RotationTrack::new(),
            scale_track: AnimationTrack::new(),
            event_track: EventTrack::new(),
        }
    }

    /// Sample all tracks at the given time.
    ///
    /// Returns `(position, rotation, scale)` where rotation is a `Quat`.
    pub fn sample(&self, time: f32) -> (Option<Vec3>, Option<Quat>, Option<Vec3>) {
        (
            self.position_track.sample(time),
            self.rotation_track.sample(time),
            self.scale_track.sample(time),
        )
    }

    /// Sample events that occurred between `prev_time` and `time`.
    pub fn sample_events(&self, prev_time: f32, time: f32, looped: bool) -> Vec<&AnimationEvent> {
        self.event_track.events_between(prev_time, time, self.duration, looped)
    }
}

/// Playback state for an animated entity.
#[derive(Debug, Clone)]
pub struct Animator {
    pub clip_name: String,
    pub time: f32,
    pub speed: f32,
    pub playing: bool,
    pub looped: bool,
}

impl Default for Animator {
    fn default() -> Self {
        Self {
            clip_name: String::new(),
            time: 0.0,
            speed: 1.0,
            playing: true,
            looped: true,
        }
    }
}

/// Delta transform extracted from root bone movement over one frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RootMotion {
    pub delta_position: Vec3,
    pub delta_rotation: Quat,
}

impl AnimationClip {
    /// Build a runtime `AnimationClip` from an asset clip.
    pub fn from_asset(asset: &rustix_asset::animation::AnimationClipAsset) -> Self {
        Self {
            name: asset.name.clone(),
            duration: asset.duration,
            position_track: AnimationTrack {
                keyframes: asset.position_track.iter().map(|k| Keyframe {
                    time: k.time,
                    value: Vec3::from(k.value),
                }).collect(),
            },
            rotation_track: RotationTrack {
                keyframes: asset.rotation_track.iter().map(|k| QuatKeyframe {
                    time: k.time,
                    value: Quat::from_euler(
                        rustix_core::math::EulerRot::XYZ,
                        k.value[0],
                        k.value[1],
                        k.value[2],
                    ),
                }).collect(),
            },
            scale_track: AnimationTrack {
                keyframes: asset.scale_track.iter().map(|k| Keyframe {
                    time: k.time,
                    value: Vec3::from(k.value),
                }).collect(),
            },
            event_track: EventTrack::new(),
        }
    }

    /// Extract root motion delta between `prev_time` and `current_time`.
    ///
    /// Returns the change in root position and rotation over the interval.
    /// Useful for moving a character controller from animation data.
    pub fn extract_root_motion(&self, prev_time: f32, current_time: f32) -> RootMotion {
        let prev_pos = self.position_track.sample(prev_time).unwrap_or(Vec3::ZERO);
        let curr_pos = self.position_track.sample(current_time).unwrap_or(Vec3::ZERO);
        let prev_rot = self.rotation_track.sample(prev_time).unwrap_or(Quat::IDENTITY);
        let curr_rot = self.rotation_track.sample(current_time).unwrap_or(Quat::IDENTITY);
        RootMotion {
            delta_position: curr_pos - prev_pos,
            delta_rotation: curr_rot * prev_rot.inverse(),
        }
    }
}

/// Convert an `AnimationAsset` into the runtime `HashMap<String, AnimationClip>`.
pub fn clips_from_asset(asset: &rustix_asset::animation::AnimationAsset) -> HashMap<String, AnimationClip> {
    let mut map = HashMap::with_capacity(asset.clip_count());
    for clip_asset in &asset.clips {
        let clip = AnimationClip::from_asset(clip_asset);
        map.insert(clip.name.clone(), clip);
    }
    map
}

/// Advance animators by `dt` and return the sampled transform values.
///
/// The caller is responsible for applying `(pos, rot, scale)` to its
/// own transform components.
pub fn update_animators(
    animators: &mut [(hecs::Entity, &mut Animator)],
    clips: &HashMap<String, AnimationClip>,
    dt: f32,
) -> Vec<(hecs::Entity, Option<Vec3>, Option<Quat>, Option<Vec3>)> {
    let mut results = Vec::with_capacity(animators.len());
    for (entity, animator) in animators {
        if !animator.playing {
            results.push((*entity, None, None, None));
            continue;
        }

        let duration = clips
            .get(&animator.clip_name)
            .map(|c| c.duration)
            .unwrap_or(0.0);

        animator.time += dt * animator.speed;
        if animator.time > duration {
            if animator.looped && duration > 0.0 {
                animator.time %= duration;
            } else {
                animator.time = duration;
                animator.playing = false;
            }
        }

        if let Some(clip) = clips.get(&animator.clip_name) {
            let (pos, rot, scale) = clip.sample(animator.time);
            results.push((*entity, pos, rot, scale));
        } else {
            results.push((*entity, None, None, None));
        }
    }
    results
}

/// Blend state for cross-fading between two animation clips.
#[derive(Debug, Clone)]
pub struct BlendAnimator {
    pub current: Animator,
    pub previous: Animator,
    pub blend_weight: f32,
    pub blend_duration: f32,
    pub blend_time: f32,
}

impl BlendAnimator {
    pub fn new(clip_name: impl Into<String>) -> Self {
        Self {
            current: Animator {
                clip_name: clip_name.into(),
                time: 0.0,
                speed: 1.0,
                playing: true,
                looped: true,
            },
            previous: Animator {
                clip_name: String::new(),
                time: 0.0,
                speed: 1.0,
                playing: false,
                looped: true,
            },
            blend_weight: 1.0,
            blend_duration: 0.0,
            blend_time: 0.0,
        }
    }

    /// Start a cross-fade to a new clip over `duration` seconds.
    pub fn transition_to(&mut self, clip_name: impl Into<String>, duration: f32) {
        std::mem::swap(&mut self.previous, &mut self.current);
        self.current = Animator {
            clip_name: clip_name.into(),
            time: 0.0,
            speed: self.current.speed,
            playing: true,
            looped: true,
        };
        self.blend_weight = 0.0;
        self.blend_duration = duration.max(0.001);
        self.blend_time = 0.0;
    }

    /// Update both animators and the blend factor.
    pub fn update(&mut self, clips: &HashMap<String, AnimationClip>, dt: f32) -> (Option<Vec3>, Option<Quat>, Option<Vec3>) {
        let current_result = sample_animator(&self.current, clips);
        let previous_result = sample_animator(&self.previous, clips);

        if self.blend_duration > 0.0 && self.blend_weight < 1.0 {
            self.blend_time += dt;
            self.blend_weight = (self.blend_time / self.blend_duration).min(1.0);
        }

        blend_results(current_result, previous_result, self.blend_weight)
    }
}

fn sample_animator(animator: &Animator, clips: &HashMap<String, AnimationClip>) -> (Option<Vec3>, Option<Quat>, Option<Vec3>) {
    if let Some(clip) = clips.get(&animator.clip_name) {
        clip.sample(animator.time)
    } else {
        (None, None, None)
    }
}

fn blend_results(
    current: (Option<Vec3>, Option<Quat>, Option<Vec3>),
    previous: (Option<Vec3>, Option<Quat>, Option<Vec3>),
    weight: f32,
) -> (Option<Vec3>, Option<Quat>, Option<Vec3>) {
    let pos = match (current.0, previous.0) {
        (Some(c), Some(p)) => Some(c.lerp(p, 1.0 - weight)),
        (Some(c), None) => Some(c),
        (None, Some(p)) => Some(p),
        (None, None) => None,
    };
    let rot = match (current.1, previous.1) {
        (Some(c), Some(p)) => Some(c.slerp(p, 1.0 - weight)),
        (Some(c), None) => Some(c),
        (None, Some(p)) => Some(p),
        (None, None) => None,
    };
    let scl = match (current.2, previous.2) {
        (Some(c), Some(p)) => Some(c.lerp(p, 1.0 - weight)),
        (Some(c), None) => Some(c),
        (None, Some(p)) => Some(p),
        (None, None) => None,
    };
    (pos, rot, scl)
}

/// Output of a single pose evaluation job.
pub type PoseOutput = (hecs::Entity, Option<Vec3>, Option<Quat>, Option<Vec3>);

/// Multi-threaded batch pose evaluator using `rayon`.
///
/// Ideal for evaluating many entity poses simultaneously on multi-core CPUs.
pub struct PoseEvaluator;

impl PoseEvaluator {
    /// Evaluate poses in parallel using `rayon::join` for pairs, or `rayon::iter` for batches.
    ///
    /// This is the simplest entry point: pass a slice of `(entity, clip_name, time)`
    /// and get back a `Vec` of sampled poses in the same order.
    ///
    /// **Example:**
    /// ```rust
    /// let inputs = vec![
    ///     (entity_a, "Run", 0.5_f32),
    ///     (entity_b, "Idle", 1.2_f32),
    /// ];
    /// let outputs = PoseEvaluator::evaluate_batch(&inputs, &clips);
    /// ```
    pub fn evaluate_batch(
        inputs: &[(hecs::Entity, &str, f32)],
        clips: &HashMap<String, AnimationClip>,
    ) -> Vec<PoseOutput> {
        use rayon::prelude::*;
        inputs
            .par_iter()
            .map(|(entity, clip_name, time)| {
                let result = if let Some(clip) = clips.get(*clip_name) {
                    clip.sample(*time)
                } else {
                    (None, None, None)
                };
                (*entity, result.0, result.1, result.2)
            })
            .collect()
    }

    /// Evaluate two poses in parallel using `rayon::join`.
    ///
    /// Slightly lower overhead than `evaluate_batch` when you only have 2 poses.
    pub fn evaluate_pair(
        a: (hecs::Entity, &str, f32),
        b: (hecs::Entity, &str, f32),
        clips: &HashMap<String, AnimationClip>,
    ) -> (PoseOutput, PoseOutput) {
        use rayon::join;
        let sample = |entity: hecs::Entity, clip_name: &str, time: f32| {
            let result = if let Some(clip) = clips.get(clip_name) {
                clip.sample(time)
            } else {
                (None, None, None)
            };
            (entity, result.0, result.1, result.2)
        };
        join(
            || sample(a.0, a.1, a.2),
            || sample(b.0, b.1, b.2),
        )
    }
}

/// Advance animators by `dt` (sequential), then sample poses in parallel.
///
/// This is a drop-in replacement for `update_animators` when you have
/// many animators and want to exploit multi-core CPUs for pose sampling.
/// Time advancement remains sequential (fast, no contention).
///
/// **Note:** This consumes the animator slice so that mutable borrows are resolved
/// before the parallel sampling phase.
pub fn update_animators_par(
    animators: &mut [(hecs::Entity, &mut Animator)],
    clips: &HashMap<String, AnimationClip>,
    dt: f32,
) -> Vec<PoseOutput> {
    // 1. Sequential time advancement (cheap, mutates state)
    let mut inputs: Vec<(hecs::Entity, String, f32)> = Vec::with_capacity(animators.len());
    for (entity, animator) in animators.iter_mut() {
        if !animator.playing {
            inputs.push((*entity, String::new(), animator.time));
            continue;
        }

        let duration = clips
            .get(&animator.clip_name)
            .map(|c| c.duration)
            .unwrap_or(0.0);

        animator.time += dt * animator.speed;
        if animator.time > duration {
            if animator.looped && duration > 0.0 {
                animator.time %= duration;
            } else {
                animator.time = duration;
                animator.playing = false;
            }
        }

        inputs.push((*entity, animator.clip_name.clone(), animator.time));
    }

    // 2. Parallel pose sampling (CPU-intensive)
    use rayon::prelude::*;
    inputs
        .par_iter()
        .map(|(entity, clip_name, time)| {
            let result = if clip_name.is_empty() {
                (None, None, None)
            } else if let Some(clip) = clips.get(clip_name) {
                clip.sample(*time)
            } else {
                (None, None, None)
            };
            (*entity, result.0, result.1, result.2)
        })
        .collect()
}
