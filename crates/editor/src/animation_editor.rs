//! Animation editor: timeline, keyframe editing, state machine graph.

use rustix_core::math::Vec3;
use rustix_core::math::Quat;

/// A keyframe on a timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe {
    pub time: f32,
    pub value: KeyframeValue,
    pub interpolation: InterpolationType,
}

/// Types of animation values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyframeValue {
    Float(f32),
    Vec3(Vec3),
    Quat(Quat),
    Bool(bool),
}

/// Interpolation between keyframes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpolationType {
    Step,
    Linear,
    Smooth,
}

/// A track of keyframes for one property.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationTrack {
    pub name: String,
    pub keyframes: Vec<Keyframe>,
}

impl AnimationTrack {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            keyframes: Vec::new(),
        }
    }

    pub fn add_keyframe(&mut self, kf: Keyframe) {
        self.keyframes.push(kf);
        self.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    pub fn remove_keyframe_at(&mut self, time: f32, tolerance: f32) {
        self.keyframes.retain(|kf| (kf.time - time).abs() > tolerance);
    }
}

/// Timeline state for the animation editor.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineState {
    pub current_time: f32,
    pub duration: f32,
    pub playing: bool,
    pub loop_playback: bool,
    pub playback_speed: f32,
    pub tracks: Vec<AnimationTrack>,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            current_time: 0.0,
            duration: 1.0,
            playing: false,
            loop_playback: true,
            playback_speed: 1.0,
            tracks: Vec::new(),
        }
    }
}

impl TimelineState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, delta: f32) {
        if !self.playing {
            return;
        }
        self.current_time += delta * self.playback_speed;
        if self.current_time > self.duration {
            if self.loop_playback {
                self.current_time %= self.duration;
            } else {
                self.current_time = self.duration;
                self.playing = false;
            }
        }
    }

    pub fn seek(&mut self, time: f32) {
        self.current_time = time.clamp(0.0, self.duration);
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.current_time = 0.0;
    }
}
