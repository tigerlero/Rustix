//! Rustix audio system for music and sound effects.
//!
//! Uses rodio for cross-platform audio playback with spatial 3D sound support.

use std::path::PathBuf;

#[cfg(feature = "audio")]
use rodio::{Sink, OutputStream, Decoder};

use thiserror::Error;
use tracing::debug;

use rustix_core::math::Vec3;

/// Unique identifier for an audio clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundId(pub u64);

/// Sound effect handle for playback control.
#[derive(Debug)]
pub struct SoundInstance {
    #[cfg(feature = "audio")]
    sink: Sink,
}

#[cfg(feature = "audio")]
impl SoundInstance {
    /// Set the volume of this sound instance (0.0 to 1.0).
    pub fn set_volume(&self, volume: f32) {
        self.sink.set_volume(volume);
    }

    /// Get the current volume.
    pub fn volume(&self) -> f32 {
        self.sink.volume()
    }

    /// Stop this sound.
    pub fn stop(&self) {
        self.sink.stop();
    }

    /// Pause this sound.
    pub fn pause(&self) {
        self.sink.pause();
    }

    /// Resume this sound.
    pub fn play(&self) {
        self.sink.play();
    }

    /// Check if this sound is playing.
    pub fn is_playing(&self) -> bool {
        !self.sink.empty()
    }
}

#[cfg(not(feature = "audio"))]
impl SoundInstance {
    pub fn set_volume(&self, _volume: f32) {}
    pub fn volume(&self) -> f32 { 0.0 }
    pub fn stop(&self) {}
    pub fn pause(&self) {}
    pub fn play(&self) {}
    pub fn is_playing(&self) -> bool { false }
}

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Audio feature not enabled")]
    NotEnabled,
    #[error("Failed to load audio file: {0}")]
    Load(String),
}

/// Main audio engine for the Rustix engine.
pub struct AudioEngine {
    #[cfg(feature = "audio")]
    stream: OutputStream,
    #[cfg(feature = "audio")]
    stream_handle: rodio::OutputStreamHandle,
    master_volume: f32,
}

impl AudioEngine {
    /// Create a new audio engine.
    pub fn new() -> Result<Self, AudioError> {
        #[cfg(feature = "audio")]
        {
            let (stream, stream_handle) = OutputStream::try_default()
                .map_err(|e| AudioError::Load(e.to_string()))?;
            
            debug!("Audio engine initialized");
            
            Ok(Self {
                stream,
                stream_handle,
                master_volume: 1.0,
            })
        }
        
        #[cfg(not(feature = "audio"))]
        {
            Err(AudioError::NotEnabled)
        }
    }

    /// Play a sound effect and return a handle to control it.
    pub fn play_sound(&self, path: &PathBuf, volume: f32, looping: bool) -> Result<SoundInstance, AudioError> {
        #[cfg(feature = "audio")]
        {
            let file = std::fs::File::open(path).map_err(|e| AudioError::Load(e.to_string()))?;
            let source = Decoder::new(file).map_err(|e| AudioError::Load(e.to_string()))?;
            
            let sink = Sink::try_new(&self.stream_handle).map_err(|e| AudioError::Load(e.to_string()))?;
            sink.set_volume(volume * self.master_volume);
            sink.set_repeat(looping);
            sink.append(source);
            sink.play();
            
            Ok(SoundInstance { sink })
        }
        
        #[cfg(not(feature = "audio"))]
        {
            Err(AudioError::NotEnabled)
        }
    }

    /// Play a sound by path (one-shot).
    pub fn play_sound_file(&self, path: &PathBuf) -> Result<(), AudioError> {
        #[cfg(feature = "audio")]
        {
            let file = std::fs::File::open(path).map_err(|e| AudioError::Load(e.to_string()))?;
            let source = Decoder::new(file).map_err(|e| AudioError::Load(e.to_string()))?;
            
            self.stream_handle.play_raw(source.convert_samples())
                .map_err(|e| AudioError::Load(e.to_string()))?;
            
            debug!("Playing sound: {:?}", path);
            Ok(())
        }
        
        #[cfg(not(feature = "audio"))]
        {
            Err(AudioError::NotEnabled)
        }
    }

    /// Play background music.
    pub fn play_music(&self, path: &PathBuf, volume: f32, looping: bool) -> Result<(), AudioError> {
        #[cfg(feature = "audio")]
        {
            let file = std::fs::File::open(path).map_err(|e| AudioError::Load(e.to_string()))?;
            let source = Decoder::new(file).map_err(|e| AudioError::Load(e.to_string()))?;
            
            let sink = Sink::try_new(&self.stream_handle).map_err(|e| AudioError::Load(e.to_string()))?;
            sink.set_volume(volume * self.master_volume);
            sink.set_repeat(looping);
            sink.append(source);
            sink.play();
            
            debug!("Playing music: {:?}", path);
            Ok(())
        }
        
        #[cfg(not(feature = "audio"))]
        {
            Err(AudioError::NotEnabled)
        }
    }

    /// Update audio state (should be called each frame).
    pub fn update(&mut self) {
        // Placeholder for per-frame updates
    }

    /// Set master volume.
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume;
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new().expect("Failed to initialize audio engine")
    }
}

/// Component for attaching sound effects to entities.
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

/// Audio listener component (usually on the main camera).
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioListener {
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
}

/// Component for spatial audio positioning.
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioSource {
    pub position: Vec3,
    pub min_distance: f32,
    pub max_distance: f32,
    pub rolloff: f32,
}