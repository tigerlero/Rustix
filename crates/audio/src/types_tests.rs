//! Tests for audio types, decoder, and waveform utilities.

use std::path::PathBuf;
use crate::*;
use crate::types::*;
use crate::waveform::*;
use crate::decoder::*;

// ---------- types.rs ----------

#[test]
fn sound_id_clone_copy() {
    let id = SoundId(42);
    let id2 = id;
    assert_eq!(id, id2);
    assert_eq!(id.0, 42);
}

#[test]
fn sound_player_default() {
    let player = SoundPlayer::default();
    assert_eq!(player.sound_path, PathBuf::new());
    assert_eq!(player.volume, 1.0);
    assert!(!player.looping);
    assert_eq!(player.spatial_blend, 0.0);
}

#[test]
fn audio_error_display() {
    let err = AudioError::PlaybackNotEnabled;
    let msg = format!("{}", err);
    assert!(msg.contains("playback"));

    let err = AudioError::Decode("test error".to_string());
    assert!(format!("{}", err).contains("test error"));
}

#[test]
fn sound_instance_decoded_samples() {
    let instance = SoundInstance {
        decoded: vec![0.1, 0.2, 0.3],
        sample_rate: 44100,
        channels: 1,
        effects: parking_lot::Mutex::new(effects::EffectChain::new()),
    };
    assert_eq!(instance.decoded_samples(), &[0.1, 0.2, 0.3]);
    assert_eq!(instance.sample_rate(), 44100);
    assert_eq!(instance.channels(), 1);
}

// ---------- decoder.rs ----------

#[test]
fn decode_from_asset_returns_samples() {
    let asset = rustix_asset::audio::AudioAsset::new(
        vec![0.5, -0.5, 0.25, -0.25],
        48000,
        2,
    );
    let (samples, sr, ch) = decode_from_asset(&asset);
    assert_eq!(samples, vec![0.5, -0.5, 0.25, -0.25]);
    assert_eq!(sr, 48000);
    assert_eq!(ch, 2);
}

// ---------- waveform.rs ----------

#[test]
fn waveform_empty() {
    let wf = generate_waveform(&[], 1, 44100, 100);
    assert!(wf.is_empty());
    assert_eq!(wf.duration, 0.0);
    assert_eq!(wf.sample_rate, 44100);
}

#[test]
fn waveform_mono_basic() {
    let samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
    let wf = generate_waveform(&samples, 1, 44100, 2);
    assert_eq!(wf.len(), 2);
    assert_eq!(wf.channels, 1);
    assert!(wf.duration > 0.0);
}

#[test]
fn waveform_bounds() {
    let samples = vec![0.5, -0.3, 0.8, -0.9];
    let wf = generate_waveform(&samples, 1, 44100, 2);
    let (min, max) = wf.bounds();
    assert!(min <= -0.3);
    assert!(max >= 0.8);
}

#[test]
fn waveform_from_instance() {
    let instance = SoundInstance {
        decoded: vec![0.2, -0.2, 0.4, -0.4],
        sample_rate: 22050,
        channels: 1,
        effects: parking_lot::Mutex::new(effects::EffectChain::new()),
    };
    let wf = generate_waveform_from_instance(&instance, 2);
    assert_eq!(wf.len(), 2);
    assert_eq!(wf.sample_rate, 22050);
}

#[test]
fn waveform_more_bars_than_samples() {
    let samples = vec![0.5, -0.5];
    let wf = generate_waveform(&samples, 1, 44100, 10);
    assert_eq!(wf.len(), 2); // one bar per sample frame
}

#[test]
fn waveform_stereo_averaging() {
    let samples = vec![1.0, 0.0, -1.0, 0.0]; // stereo: [L,R,L,R]
    let wf = generate_waveform(&samples, 2, 44100, 2);
    // Each frame averages to 0.5, then -0.5
    assert_eq!(wf.bars[0].min, 0.5);
    assert_eq!(wf.bars[0].max, 0.5);
    assert_eq!(wf.bars[1].min, -0.5);
    assert_eq!(wf.bars[1].max, -0.5);
}

// ---------- lib.rs re-exports ----------

#[test]
fn re_exported_constants() {
    assert_eq!(spatial::REFERENCE_DISTANCE, 1.0);
    assert_eq!(spatial::SPEED_OF_SOUND, 343.0);
    assert!(spatial::HEAD_RADIUS > 0.0);
    assert!(spatial::MAX_ITD > 0.0);
}
