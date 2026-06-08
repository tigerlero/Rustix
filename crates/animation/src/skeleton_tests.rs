//! Tests for skeleton, bone, and retargeting.

use rustix_core::math::{Vec3, Mat4, Quat, EulerRot};
use crate::skeleton::{Bone, Skeleton};

fn bone(name: &str, parent: u16, pos: Vec3, rot: Vec3, scl: Vec3) -> Bone {
    let mut name_arr = [0u8; 32];
    let bytes = name.as_bytes();
    let len = bytes.len().min(32);
    name_arr[..len].copy_from_slice(&bytes[..len]);
    Bone {
        name: name_arr,
        parent,
        local_pos: pos,
        local_rot: rot,
        local_scl: scl,
        inverse_bind: Mat4::IDENTITY,
    }
}

#[test]
fn bone_name_str() {
    let b = bone("root", u16::MAX, Vec3::ZERO, Vec3::ZERO, Vec3::ONE);
    assert_eq!(b.name_str(), "root");
}

#[test]
fn bone_name_str_long() {
    let name = "a".repeat(40);
    let mut name_arr = [0u8; 32];
    let bytes = name.as_bytes();
    let len = bytes.len().min(32);
    name_arr[..len].copy_from_slice(&bytes[..len]);
    let b = Bone {
        name: name_arr,
        parent: u16::MAX,
        local_pos: Vec3::ZERO,
        local_rot: Vec3::ZERO,
        local_scl: Vec3::ONE,
        inverse_bind: Mat4::IDENTITY,
    };
    assert_eq!(b.name_str().len(), 32);
}

#[test]
fn skeleton_new_and_count() {
    let bones = vec![
        bone("root", u16::MAX, Vec3::ZERO, Vec3::ZERO, Vec3::ONE),
    ];
    let skel = Skeleton::new(bones);
    assert_eq!(skel.bone_count(), 1);
}

#[test]
fn skeleton_find_bone_index_found() {
    let bones = vec![
        bone("root", u16::MAX, Vec3::ZERO, Vec3::ZERO, Vec3::ONE),
        bone("child", 0, Vec3::X, Vec3::ZERO, Vec3::ONE),
    ];
    let skel = Skeleton::new(bones);
    assert_eq!(skel.find_bone_index("root"), Some(0));
    assert_eq!(skel.find_bone_index("child"), Some(1));
}

#[test]
fn skeleton_find_bone_index_missing() {
    let skel = Skeleton::new(vec![bone("root", u16::MAX, Vec3::ZERO, Vec3::ZERO, Vec3::ONE)]);
    assert_eq!(skel.find_bone_index("missing"), None);
}

#[test]
fn skeleton_compute_world_matrices_root_only() {
    let skel = Skeleton::new(vec![
        bone("root", u16::MAX, Vec3::new(1.0, 2.0, 3.0), Vec3::ZERO, Vec3::ONE),
    ]);
    let mats = skel.compute_world_matrices();
    assert_eq!(mats.len(), 1);
    let (_, _, pos) = mats[0].to_scale_rotation_translation();
    assert!((pos - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-4);
}

#[test]
fn skeleton_compute_world_matrices_parent_child() {
    let skel = Skeleton::new(vec![
        bone("root", u16::MAX, Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::ONE),
        bone("child", 0, Vec3::new(0.0, 2.0, 0.0), Vec3::ZERO, Vec3::ONE),
    ]);
    let mats = skel.compute_world_matrices();
    assert_eq!(mats.len(), 2);
    let (_, _, child_pos) = mats[1].to_scale_rotation_translation();
    assert!((child_pos - Vec3::new(1.0, 2.0, 0.0)).length() < 1e-4);
}

#[test]
fn skeleton_compute_world_matrices_with_scale() {
    let skel = Skeleton::new(vec![
        bone("root", u16::MAX, Vec3::ZERO, Vec3::ZERO, Vec3::splat(2.0)),
        bone("child", 0, Vec3::ONE, Vec3::ZERO, Vec3::ONE),
    ]);
    let mats = skel.compute_world_matrices();
    let (scale, _, _) = mats[1].to_scale_rotation_translation();
    assert!((scale - Vec3::splat(2.0)).length() < 1e-4);
}

#[test]
fn skeleton_compute_world_matrices_with_rotation() {
    let skel = Skeleton::new(vec![
        bone("root", u16::MAX, Vec3::ZERO, Vec3::new(0.0, 90f32.to_radians(), 0.0), Vec3::ONE),
        bone("child", 0, Vec3::X, Vec3::ZERO, Vec3::ONE),
    ]);
    let mats = skel.compute_world_matrices();
    let (_, _, child_pos) = mats[1].to_scale_rotation_translation();
    // After Y rotation of 90 degrees, X axis points to -Z
    assert!((child_pos - (-Vec3::Z)).length() < 1e-3);
}

#[test]
fn skeleton_compute_skinning_matrices_identity_bind() {
    let skel = Skeleton::new(vec![
        bone("root", u16::MAX, Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::ONE),
    ]);
    let skinning = skel.compute_skinning_matrices();
    let world = skel.compute_world_matrices();
    // inverse_bind is identity, so skinning = world
    let s = skinning[0].to_cols_array();
    let w = world[0].to_cols_array();
    for i in 0..16 {
        assert!((s[i] - w[i]).abs() < 1e-4, "element {} differs: {} vs {}", i, s[i], w[i]);
    }
}

#[test]
fn skeleton_retarget_from_copies_rotations() {
    let mut target = Skeleton::new(vec![
        bone("hip", u16::MAX, Vec3::ZERO, Vec3::new(0.1, 0.0, 0.0), Vec3::ONE),
        bone("knee", 0, Vec3::Y, Vec3::new(0.2, 0.0, 0.0), Vec3::ONE),
    ]);
    let source = Skeleton::new(vec![
        bone("hip", u16::MAX, Vec3::ZERO, Vec3::new(0.5, 0.0, 0.0), Vec3::ONE),
        bone("knee", 0, Vec3::Y, Vec3::new(0.6, 0.0, 0.0), Vec3::ONE),
    ]);

    target.retarget_from(&source);
    assert!((target.bones[0].local_rot - Vec3::new(0.5, 0.0, 0.0)).length() < 1e-4);
    assert!((target.bones[1].local_rot - Vec3::new(0.6, 0.0, 0.0)).length() < 1e-4);
    // Positions and scales should remain unchanged
    assert_eq!(target.bones[0].local_pos, Vec3::ZERO);
    assert_eq!(target.bones[1].local_pos, Vec3::Y);
}

#[test]
fn skeleton_retarget_from_ignores_missing_bones() {
    let mut target = Skeleton::new(vec![
        bone("hip", u16::MAX, Vec3::ZERO, Vec3::new(0.1, 0.0, 0.0), Vec3::ONE),
        bone("knee", 0, Vec3::Y, Vec3::new(0.2, 0.0, 0.0), Vec3::ONE),
    ]);
    let source = Skeleton::new(vec![
        bone("hip", u16::MAX, Vec3::ZERO, Vec3::new(0.5, 0.0, 0.0), Vec3::ONE),
        // no "knee" bone in source
    ]);

    target.retarget_from(&source);
    // hip should be updated, knee should remain unchanged
    assert!((target.bones[0].local_rot - Vec3::new(0.5, 0.0, 0.0)).length() < 1e-4);
    assert!((target.bones[1].local_rot - Vec3::new(0.2, 0.0, 0.0)).length() < 1e-4);
}

#[test]
fn skeleton_retargeted_world_matrices() {
    let mut target = Skeleton::new(vec![
        bone("root", u16::MAX, Vec3::ZERO, Vec3::ZERO, Vec3::ONE),
    ]);
    let source = Skeleton::new(vec![
        bone("root", u16::MAX, Vec3::ZERO, Vec3::new(0.0, 90f32.to_radians(), 0.0), Vec3::ONE),
    ]);

    let mats = target.retargeted_world_matrices(&source);
    let (_, _, pos) = mats[0].to_scale_rotation_translation();
    // With Y rotation of 90 degrees, the world matrix should have that rotation
    let (_, rot, _) = mats[0].to_scale_rotation_translation();
    assert_ne!(rot, Quat::IDENTITY);
}

#[test]
fn skeleton_retargeted_skinning_matrices() {
    let mut target = Skeleton::new(vec![
        bone("root", u16::MAX, Vec3::ZERO, Vec3::ZERO, Vec3::ONE),
    ]);
    let source = Skeleton::new(vec![
        bone("root", u16::MAX, Vec3::ZERO, Vec3::new(0.0, 90f32.to_radians(), 0.0), Vec3::ONE),
    ]);

    let skinning = target.retargeted_skinning_matrices(&source);
    assert_eq!(skinning.len(), 1);
}

#[test]
fn skeleton_empty() {
    let skel = Skeleton::new(vec![]);
    assert_eq!(skel.bone_count(), 0);
    assert!(skel.compute_world_matrices().is_empty());
    assert!(skel.compute_skinning_matrices().is_empty());
}

#[test]
fn skeleton_deep_hierarchy() {
    let skel = Skeleton::new(vec![
        bone("a", u16::MAX, Vec3::X, Vec3::ZERO, Vec3::ONE),
        bone("b", 0, Vec3::Y, Vec3::ZERO, Vec3::ONE),
        bone("c", 1, Vec3::Z, Vec3::ZERO, Vec3::ONE),
    ]);
    let mats = skel.compute_world_matrices();
    assert_eq!(mats.len(), 3);
    let (_, _, c_pos) = mats[2].to_scale_rotation_translation();
    assert!((c_pos - Vec3::new(1.0, 1.0, 1.0)).length() < 1e-4);
}

#[test]
fn bone_clone_copy() {
    let b = bone("test", u16::MAX, Vec3::ONE, Vec3::ZERO, Vec3::ONE);
    let b2 = b;
    assert_eq!(b.name_str(), b2.name_str());
}

#[test]
fn skeleton_clone_partial_eq() {
    let skel = Skeleton::new(vec![bone("root", u16::MAX, Vec3::ZERO, Vec3::ZERO, Vec3::ONE)]);
    let skel2 = skel.clone();
    assert_eq!(skel, skel2);
}
