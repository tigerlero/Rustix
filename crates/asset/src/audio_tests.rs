//! Tests for audio asset types and binary format.

use crate::audio::{AudioAsset, import_rxsound, export_rxsound};

#[test]
fn audio_asset_new() {
    let samples = vec![0.0f32, 0.5, 1.0, 0.5];
    let asset = AudioAsset::new(samples.clone(), 44100, 2);
    assert_eq!(asset.samples, samples);
    assert_eq!(asset.sample_rate, 44100);
    assert_eq!(asset.channels, 2);
    assert_eq!(asset.frame_count(), 2);
    assert!((asset.duration_seconds - (2.0 / 44100.0)).abs() < 1e-6);
}

#[test]
fn audio_asset_empty() {
    let asset = AudioAsset::new(vec![], 44100, 2);
    assert!(asset.samples.is_empty());
    assert_eq!(asset.frame_count(), 0);
    assert_eq!(asset.duration_seconds, 0.0);
}

#[test]
fn audio_asset_zero_channels() {
    let asset = AudioAsset::new(vec![0.5], 44100, 0);
    assert_eq!(asset.frame_count(), 0);
    assert_eq!(asset.duration_seconds, 0.0);
}

#[test]
fn rxsound_roundtrip() {
    let samples = vec![0.0f32, 0.25, 0.5, 0.75, 1.0, -1.0];
    let original = AudioAsset::new(samples, 48000, 2);
    let bytes = export_rxsound(&original);
    let imported = import_rxsound(&bytes).unwrap();
    assert_eq!(imported.sample_rate, original.sample_rate);
    assert_eq!(imported.channels, original.channels);
    assert_eq!(imported.samples, original.samples);
    assert!((imported.duration_seconds - original.duration_seconds).abs() < 1e-6);
}

#[test]
fn rxsound_empty_roundtrip() {
    let original = AudioAsset::new(vec![], 44100, 2);
    let bytes = export_rxsound(&original);
    let imported = import_rxsound(&bytes).unwrap();
    assert!(imported.samples.is_empty());
    assert_eq!(imported.sample_rate, 44100);
    assert_eq!(imported.channels, 2);
}

#[test]
fn rxsound_invalid_magic() {
    let result = import_rxsound(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxsound_too_small() {
    let result = import_rxsound(b"RXD1");
    assert!(result.is_err());
}

#[test]
fn rxsound_unsupported_version() {
    let mut bytes = b"RXD1".to_vec();
    bytes.extend_from_slice(&99u32.to_le_bytes());
    let result = import_rxsound(&bytes);
    assert!(result.is_err());
}
