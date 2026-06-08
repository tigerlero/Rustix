use std::path::Path;

use tracing::debug;

#[cfg(feature = "audio-playback")]
use tracing::warn;

use parking_lot::Mutex;

use crate::decoder::decode_audio;
use crate::effects::EffectChain;
use crate::spatial::{calculate_attenuation, calculate_horiz_azimuth, hrtf_panning, AudioListener, AudioSource};
use crate::stream::StreamDecoder;
use crate::{AudioError, SoundInstance};

pub struct AudioEngine {
    #[cfg(feature = "audio-playback")]
    stream: Option<rodio::OutputStream>,
    #[cfg(feature = "audio-playback")]
    stream_handle: Option<rodio::OutputStreamHandle>,
    master_volume: f32,
    playback_available: bool,
    listener: AudioListener,
    /// Currently playing preview instance (for Asset Browser etc).
    preview: Option<SoundInstance>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, AudioError> {
        #[cfg(feature = "audio-playback")]
        {
            match rodio::OutputStream::try_default() {
                Ok((stream, handle)) => {
                    debug!("audio playback initialized");
                    return Ok(Self { stream: Some(stream), stream_handle: Some(handle), master_volume: 1.0, playback_available: true, listener: AudioListener::default(), preview: None });
                }
                Err(e) => {
                    warn!("audio playback unavailable: {e}");
                    return Ok(Self { stream: None, stream_handle: None, master_volume: 1.0, playback_available: false, listener: AudioListener::default(), preview: None });
                }
            }
        }
        #[cfg(not(feature = "audio-playback"))]
        {
            debug!("audio engine initialized (playback not enabled)");
            Ok(Self { master_volume: 1.0, playback_available: false, listener: AudioListener::default(), preview: None })
        }
    }

    pub fn set_listener(&mut self, listener: AudioListener) {
        self.listener = listener;
    }

    pub fn listener(&self) -> &AudioListener {
        &self.listener
    }

    pub fn play_sound(&self, path: &Path, _volume: f32, _looping: bool) -> Result<SoundInstance, AudioError> {
        let (decoded, sample_rate, channels) = decode_audio(path)?;

        #[cfg(feature = "audio-playback")]
        if self.playback_available {
            if let Some(ref handle) = self.stream_handle {
                let source = rodio::buffer::SamplesBuffer::new(
                    channels,
                    sample_rate,
                    decoded.clone(),
                );
                let sink = rodio::Sink::try_new(handle)
                    .map_err(|e| AudioError::Decode(e.to_string()))?;
                sink.set_volume(volume * self.master_volume);
                sink.set_repeat(looping);
                sink.append(source);
                sink.play();
                return Ok(SoundInstance { sink, decoded, sample_rate, channels, effects: parking_lot::Mutex::new(EffectChain::new()) });
            }
        }

        Ok(SoundInstance { decoded, sample_rate, channels, effects: Mutex::new(EffectChain::new()) })
    }

    pub fn update(&mut self) {}

    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume;
    }

    pub fn master_volume(&self) -> f32 { self.master_volume }
    pub fn is_playback_available(&self) -> bool { self.playback_available }

    pub fn play_sound_file(&self, path: &Path) -> Result<SoundInstance, AudioError> {
        self.play_sound(path, 1.0, false)
    }

    // ── Asset Browser preview ──

    /// Play a one-shot preview of an audio file, stopping any previous preview.
    ///
    /// Used by the Asset Browser (or any editor UI) to audition sounds
    /// without creating a persistent `SoundInstance` that the caller must track.
    pub fn preview(&mut self, path: &Path) -> Result<(), AudioError> {
        self.stop_preview();
        let instance = self.play_sound(path, 1.0, false)?;
        self.preview = Some(instance);
        Ok(())
    }

    /// Stop the currently playing preview, if any.
    pub fn stop_preview(&mut self) {
        if let Some(ref instance) = self.preview {
            instance.stop();
        }
        self.preview = None;
    }

    /// Whether a preview is currently audible.
    pub fn is_previewing(&self) -> bool {
        self.preview.as_ref().map(|p| p.is_playing()).unwrap_or(false)
    }

    pub fn play_asset(&self, asset: &rustix_asset::audio::AudioAsset, _volume: f32, _looping: bool) -> Result<SoundInstance, AudioError> {
        let decoded = asset.samples.clone();
        let sample_rate = asset.sample_rate;
        let channels = asset.channels;

        #[cfg(feature = "audio-playback")]
        if self.playback_available {
            if let Some(ref handle) = self.stream_handle {
                let source = rodio::buffer::SamplesBuffer::new(
                    channels,
                    sample_rate,
                    decoded.clone(),
                );
                let sink = rodio::Sink::try_new(handle)
                    .map_err(|e| AudioError::Decode(e.to_string()))?;
                sink.set_volume(volume * self.master_volume);
                sink.set_repeat(looping);
                sink.append(source);
                sink.play();
                return Ok(SoundInstance { sink, decoded, sample_rate, channels, effects: parking_lot::Mutex::new(EffectChain::new()) });
            }
        }

        Ok(SoundInstance { decoded, sample_rate, channels, effects: parking_lot::Mutex::new(EffectChain::new()) })
    }

    pub fn open_stream(&self, path: &Path) -> Result<StreamDecoder, AudioError> {
        StreamDecoder::open(path)
    }

    #[cfg(feature = "audio-playback")]
    pub fn stream_sound(&self, path: &Path, volume: f32, looping: bool) -> Result<StreamingInstance, AudioError> {
        let decoder = StreamDecoder::open(path)?;
        let sr = decoder.sample_rate();
        let ch = decoder.channels();

        if self.playback_available {
            if let Some(ref handle) = self.stream_handle {
                let source = StreamingSource { decoder, looping, ended: false };
                let sink = rodio::Sink::try_new(handle)
                    .map_err(|e| AudioError::Decode(e.to_string()))?;
                sink.set_volume(volume * self.master_volume);
                sink.append(source);
                sink.play();
                return Ok(StreamingInstance { sink: Some(sink), sample_rate: sr, channels: ch });
            }
        }
        Err(AudioError::PlaybackNotEnabled)
    }

    #[cfg(not(feature = "audio-playback"))]
    pub fn stream_sound(&self, _path: &Path, _volume: f32, _looping: bool) -> Result<StreamingInstance, AudioError> {
        Err(AudioError::PlaybackNotEnabled)
    }

    pub fn play_sound_spatial(
        &self,
        path: &Path,
        source: AudioSource,
        spatial_blend: f32,
        #[allow(unused_variables)] looping: bool,
    ) -> Result<SoundInstance, AudioError> {
        let (mut decoded, sample_rate, channels) = decode_audio(path)?;

        let attenuation = calculate_attenuation(
            source.position.distance(self.listener.position),
            source.min_distance,
            source.max_distance,
            source.rolloff,
        );

        let angle = calculate_horiz_azimuth(
            self.listener.position,
            self.listener.forward,
            source.position,
        );
        let (left_gain, right_gain) = hrtf_panning(angle);

        match channels {
            1 => {
                if spatial_blend > 0.0 && attenuation > 0.0 {
                    let gain = (left_gain + right_gain) * 0.5;
                    for sample in decoded.iter_mut() {
                        *sample *= gain * attenuation * self.master_volume;
                    }
                } else {
                    for sample in decoded.iter_mut() {
                        *sample *= attenuation * self.master_volume;
                    }
                }
            }
            2 => {
                if spatial_blend > 0.0 && attenuation > 0.0 {
                    for chunk in decoded.chunks_mut(2) {
                        if chunk.len() >= 2 {
                            chunk[0] *= left_gain * attenuation * self.master_volume;
                            chunk[1] *= right_gain * attenuation * self.master_volume;
                        }
                    }
                } else {
                    for sample in decoded.iter_mut() {
                        *sample *= attenuation * self.master_volume;
                    }
                }
            }
            _ => {
                for sample in decoded.iter_mut() {
                    *sample *= attenuation * self.master_volume;
                }
            }
        }

        #[cfg(feature = "audio-playback")]
        if self.playback_available {
            if let Some(ref handle) = self.stream_handle {
                let source = rodio::buffer::SamplesBuffer::new(
                    channels,
                    sample_rate,
                    decoded.clone(),
                );
                let sink = rodio::Sink::try_new(handle)
                    .map_err(|e| AudioError::Decode(e.to_string()))?;
                sink.set_volume(1.0);
                sink.set_repeat(looping);
                sink.append(source);
                sink.play();
                return Ok(SoundInstance { sink, decoded, sample_rate, channels, effects: Mutex::new(EffectChain::new()) });
            }
        }

        Ok(SoundInstance { decoded, sample_rate, channels, effects: Mutex::new(EffectChain::new()) })
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new().expect("failed to init audio engine")
    }
}

pub struct StreamingInstance {
    #[cfg(feature = "audio-playback")]
    sink: Option<rodio::Sink>,
    sample_rate: u32,
    channels: u16,
}

impl StreamingInstance {
    pub fn sample_rate(&self) -> u32 { self.sample_rate }
    pub fn channels(&self) -> u16 { self.channels }

    #[cfg(feature = "audio-playback")]
    pub fn set_volume(&self, v: f32) { if let Some(ref s) = self.sink { s.set_volume(v); } }
    #[cfg(not(feature = "audio-playback"))]
    pub fn set_volume(&self, _v: f32) {}

    #[cfg(feature = "audio-playback")]
    pub fn stop(&self) { if let Some(ref s) = self.sink { s.stop(); } }
    #[cfg(not(feature = "audio-playback"))]
    pub fn stop(&self) {}

    #[cfg(feature = "audio-playback")]
    pub fn pause(&self) { if let Some(ref s) = self.sink { s.pause(); } }
    #[cfg(not(feature = "audio-playback"))]
    pub fn pause(&self) {}

    #[cfg(feature = "audio-playback")]
    pub fn is_playing(&self) -> bool { self.sink.as_ref().map(|s| !s.empty()).unwrap_or(false) }
    #[cfg(not(feature = "audio-playback"))]
    pub fn is_playing(&self) -> bool { false }
}

#[cfg(feature = "audio-playback")]
struct StreamingSource {
    decoder: StreamDecoder,
    looping: bool,
    ended: bool,
}

#[cfg(feature = "audio-playback")]
impl Iterator for StreamingSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.ended {
            if self.looping { let _ = self.decoder.seek(0.0); self.ended = false; }
            else { return None; }
        }
        let mut sample = 0.0f32;
        let n = self.decoder.read(std::slice::from_mut(&mut sample));
        if n == 0 {
            if self.looping { let _ = self.decoder.seek(0.0); let n2 = self.decoder.read(std::slice::from_mut(&mut sample)); if n2 == 0 { return None; } }
            else { self.ended = true; return None; }
        }
        Some(sample)
    }
}

#[cfg(feature = "audio-playback")]
impl rodio::Source for StreamingSource {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { self.decoder.channels() }
    fn sample_rate(&self) -> u32 { self.decoder.sample_rate() }
    fn total_duration(&self) -> Option<std::time::Duration> { None }
}