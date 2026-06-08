//! Tests for skeleton asset types and binary format.

use rustix_core::math::Mat4;
use crate::skeleton::{BoneAsset, SkeletonAsset, import_rxskel, export_rxskel};

#[test]
fn bone_asset_new() {
    let bone = BoneAsset::new("root", u16::MAX);
    let name_str = std::str::from_utf8(&bone.name).unwrap().trim_end_matches('\0');
    assert_eq!(name_str, "root");
    assert_eq!(bone.parent, u16::MAX);
    assert_eq!(bone.local_pos, [0.0; 3]);
    assert_eq!(bone.local_rot, [0.0; 3]);
    assert_eq!(bone.local_scl, [1.0; 3]);
    assert_eq!(bone.inverse_bind, Mat4::IDENTITY.to_cols_array_2d());
}

#[test]
fn bone_asset_new_long_name() {
    let name = "a".repeat(40);
    let bone = BoneAsset::new(&name, 0);
    // Name should be truncated to 32 bytes
    let name_str = std::str::from_utf8(&bone.name).unwrap().trim_end_matches('\0');
    assert_eq!(name_str.len(), 32);
}

#[test]
fn skeleton_asset_new_and_count() {
    let bones = vec![
        BoneAsset::new("root", u16::MAX),
        BoneAsset::new("child", 0),
    ];
    let skel = SkeletonAsset::new(bones);
    assert_eq!(skel.bone_count(), 2);
}

#[test]
fn skeleton_asset_find_bone_index_found() {
    let bones = vec![
        BoneAsset::new("hip", u16::MAX),
        BoneAsset::new("knee", 0),
    ];
    let skel = SkeletonAsset::new(bones);
    assert_eq!(skel.find_bone_index("hip"), Some(0));
    assert_eq!(skel.find_bone_index("knee"), Some(1));
}

#[test]
fn skeleton_asset_find_bone_index_missing() {
    let skel = SkeletonAsset::new(vec![BoneAsset::new("root", u16::MAX)]);
    assert_eq!(skel.find_bone_index("missing"), None);
}

#[test]
fn skeleton_asset_empty() {
    let skel = SkeletonAsset::new(vec![]);
    assert_eq!(skel.bone_count(), 0);
    assert_eq!(skel.find_bone_index("anything"), None);
}

#[test]
fn rxskel_roundtrip() {
    let mut bone0 = BoneAsset::new("root", u16::MAX);
    bone0.local_pos = [1.0, 2.0, 3.0];
    bone0.local_rot = [0.1, 0.2, 0.3];
    bone0.local_scl = [1.5, 1.5, 1.5];

    let mut bone1 = BoneAsset::new("child", 0);
    bone1.local_pos = [0.0, 1.0, 0.0];

    let original = SkeletonAsset::new(vec![bone0, bone1]);
    let bytes = export_rxskel(&original);
    let imported = import_rxskel(&bytes).unwrap();
    assert_eq!(imported.bone_count(), 2);

    let b0 = &imported.bones[0];
    let name_str = std::str::from_utf8(&b0.name).unwrap().trim_end_matches('\0');
    assert_eq!(name_str, "root");
    assert_eq!(b0.parent, u16::MAX);
    assert_eq!(b0.local_pos, [1.0, 2.0, 3.0]);
    assert_eq!(b0.local_rot, [0.1, 0.2, 0.3]);
    assert_eq!(b0.local_scl, [1.5, 1.5, 1.5]);

    let b1 = &imported.bones[1];
    assert_eq!(b1.parent, 0);
    assert_eq!(b1.local_pos, [0.0, 1.0, 0.0]);
}

#[test]
fn rxskel_empty_roundtrip() {
    let original = SkeletonAsset::new(vec![]);
    let bytes = export_rxskel(&original);
    let imported = import_rxskel(&bytes).unwrap();
    assert_eq!(imported.bone_count(), 0);
}

#[test]
fn rxskel_invalid_magic() {
    let result = import_rxskel(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxskel_too_small() {
    let result = import_rxskel(b"RXK1");
    assert!(result.is_err());
}

#[test]
fn rxskel_unsupported_version() {
    let mut bytes = b"RXK1".to_vec();
    bytes.extend_from_slice(&99u32.to_le_bytes());
    let result = import_rxskel(&bytes);
    assert!(result.is_err());
}
