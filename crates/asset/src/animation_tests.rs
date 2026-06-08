//! Tests for animation asset types and binary format.

use crate::animation::{KeyframeAsset, AnimationClipAsset, AnimationAsset, import_rxanim, export_rxanim};

#[test]
fn keyframe_asset_new() {
    let kf = KeyframeAsset::new(0.5, [1.0, 2.0, 3.0]);
    assert_eq!(kf.time, 0.5);
    assert_eq!(kf.value, [1.0, 2.0, 3.0]);
}

#[test]
fn animation_clip_asset_new() {
    let clip = AnimationClipAsset::new("idle", 2.5);
    assert_eq!(clip.name, "idle");
    assert_eq!(clip.duration, 2.5);
    assert!(clip.position_track.is_empty());
    assert!(clip.rotation_track.is_empty());
    assert!(clip.scale_track.is_empty());
}

#[test]
fn animation_asset_new_and_count() {
    let clip = AnimationClipAsset::new("run", 1.0);
    let asset = AnimationAsset::new(vec![clip]);
    assert_eq!(asset.clip_count(), 1);
}

#[test]
fn animation_asset_empty() {
    let asset = AnimationAsset::new(vec![]);
    assert_eq!(asset.clip_count(), 0);
}

#[test]
fn rxanim_roundtrip() {
    let mut clip = AnimationClipAsset::new("run", 1.5);
    clip.position_track.push(KeyframeAsset::new(0.0, [0.0, 0.0, 0.0]));
    clip.position_track.push(KeyframeAsset::new(1.0, [1.0, 0.0, 0.0]));
    clip.rotation_track.push(KeyframeAsset::new(0.0, [0.0, 0.0, 0.0]));
    clip.scale_track.push(KeyframeAsset::new(0.0, [1.0, 1.0, 1.0]));

    let original = AnimationAsset::new(vec![clip]);
    let bytes = export_rxanim(&original);
    let imported = import_rxanim(&bytes).unwrap();

    assert_eq!(imported.clip_count(), 1);
    let imported_clip = &imported.clips[0];
    assert_eq!(imported_clip.name, "run");
    assert_eq!(imported_clip.duration, 1.5);
    assert_eq!(imported_clip.position_track.len(), 2);
    assert_eq!(imported_clip.rotation_track.len(), 1);
    assert_eq!(imported_clip.scale_track.len(), 1);
    assert_eq!(imported_clip.position_track[0].time, 0.0);
    assert_eq!(imported_clip.position_track[0].value, [0.0, 0.0, 0.0]);
    assert_eq!(imported_clip.position_track[1].value, [1.0, 0.0, 0.0]);
}

#[test]
fn rxanim_empty_roundtrip() {
    let original = AnimationAsset::new(vec![]);
    let bytes = export_rxanim(&original);
    let imported = import_rxanim(&bytes).unwrap();
    assert_eq!(imported.clip_count(), 0);
}

#[test]
fn rxanim_invalid_magic() {
    let result = import_rxanim(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxanim_too_small() {
    let result = import_rxanim(b"RXN1");
    assert!(result.is_err());
}

#[test]
fn rxanim_unsupported_version() {
    let mut bytes = b"RXN1".to_vec();
    bytes.extend_from_slice(&99u32.to_le_bytes());
    let result = import_rxanim(&bytes);
    assert!(result.is_err());
}
