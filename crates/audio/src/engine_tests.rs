//! Tests for audio engine state.

use crate::engine::AudioEngine;
use crate::spatial::AudioListener;
use rustix_core::math::Vec3;

#[test]
fn audio_engine_new_ok() {
    let engine = AudioEngine::new();
    assert!(engine.is_ok());
}

#[test]
fn audio_engine_default_ok() {
    let engine: AudioEngine = Default::default();
    assert_eq!(engine.master_volume(), 1.0);
}

#[test]
fn audio_engine_master_volume() {
    let mut engine: AudioEngine = Default::default();
    assert_eq!(engine.master_volume(), 1.0);
    engine.set_master_volume(0.5);
    assert_eq!(engine.master_volume(), 0.5);
}

#[test]
fn audio_engine_playback_not_available_without_feature() {
    let engine: AudioEngine = Default::default();
    assert!(!engine.is_playback_available());
}

#[test]
fn audio_engine_listener_default() {
    let engine: AudioEngine = Default::default();
    let listener = engine.listener();
    assert_eq!(listener.position, Vec3::ZERO);
}

#[test]
fn audio_engine_set_listener() {
    let mut engine: AudioEngine = Default::default();
    let new_listener = AudioListener {
        position: Vec3::X,
        forward: Vec3::NEG_Z,
        up: Vec3::Y,
    };
    engine.set_listener(new_listener);
    assert_eq!(engine.listener().position, Vec3::X);
}
