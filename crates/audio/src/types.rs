use std::path::PathBuf;

use parking_lot::Mutex;
use thiserror::Error;

use crate::effects::EffectChain;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundId(pub u64);

#[derive(Debug)]
pub struct SoundInstance {
    #[cfg(feature = "audio-playback")]
    pub(crate) sink: rodio::Sink,
    pub decoded: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub effects: Mutex<EffectChain>,
}

impl SoundInstance {
    pub fn set_volume(&self, _volume: f32) {
        #[cfg(feature = "audio-playback")]
        self.sink.set_volume(volume);
    }

    pub fn volume(&self) -> f32 {
        #[cfg(feature = "audio-playback")]
        { self.sink.volume() }
        #[cfg(not(feature = "audio-playback"))]
        { 0.0 }
    }

    pub fn stop(&self) {
        #[cfg(feature = "audio-playback")]
        self.sink.stop();
    }

    pub fn pause(&self) {
        #[cfg(feature = "audio-playback")]
        self.sink.pause();
    }

    pub fn play(&self) {
        #[cfg(feature = "audio-playback")]
        self.sink.play();
    }

    pub fn is_playing(&self) -> bool {
        #[cfg(feature = "audio-playback")]
        { !self.sink.empty() }
        #[cfg(not(feature = "audio-playback"))]
        { false }
    }

    pub fn decoded_samples(&self) -> &[f32] { &self.decoded }
    pub fn sample_rate(&self) -> u32 { self.sample_rate }
    pub fn channels(&self) -> u16 { self.channels }
}

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("audio playback not enabled (build with --features audio-playback)")]
    PlaybackNotEnabled,
    #[error("failed to open file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to decode audio: {0}")]
    Decode(String),
}

#[derive(Debug, Clone)]
pub struct SoundPlayer {
    pub sound_path: PathBuf,
    pub volume: f32,
    pub looping: bool,
    pub spatial_blend: f32,
}

impl Default for SoundPlayer {
    fn default() -> Self {
        Self {
            sound_path: PathBuf::new(),
            volume: 1.0,
            looping: false,
            spatial_blend: 0.0,
        }
    }
}