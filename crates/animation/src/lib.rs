use rustix_core::math::Vec3;
use std::collections::HashMap;

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

/// A named animation clip composed of position, rotation, and scale tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    pub position_track: AnimationTrack,
    pub rotation_track: AnimationTrack,
    pub scale_track: AnimationTrack,
}

impl AnimationClip {
    pub fn new(name: impl Into<String>, duration: f32) -> Self {
        Self {
            name: name.into(),
            duration,
            position_track: AnimationTrack::new(),
            rotation_track: AnimationTrack::new(),
            scale_track: AnimationTrack::new(),
        }
    }

    /// Sample all tracks at the given time.
    pub fn sample(&self, time: f32) -> (Option<Vec3>, Option<Vec3>, Option<Vec3>) {
        (
            self.position_track.sample(time),
            self.rotation_track.sample(time),
            self.scale_track.sample(time),
        )
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

/// Advance animators by `dt` and return the sampled transform values.
///
/// The caller is responsible for applying `(pos, rot, scale)` to its
/// own transform components.
pub fn update_animators(
    animators: &mut [(hecs::Entity, &mut Animator)],
    clips: &HashMap<String, AnimationClip>,
    dt: f32,
) -> Vec<(hecs::Entity, Option<Vec3>, Option<Vec3>, Option<Vec3>)> {
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
