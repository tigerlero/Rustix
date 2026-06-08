//! Runtime skeleton types for skinning and bone animation.

use rustix_core::math::{Vec3, Mat4};

/// A single bone in a skeleton hierarchy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bone {
    /// Bone name (for debugging and animation targeting).
    pub name: [u8; 32],
    /// Parent bone index, or `u16::MAX` for the root.
    pub parent: u16,
    /// Local position relative to parent.
    pub local_pos: Vec3,
    /// Local rotation (Euler angles XYZ).
    pub local_rot: Vec3,
    /// Local scale.
    pub local_scl: Vec3,
    /// Inverse bind matrix (transforms from model space to bone-local space).
    pub inverse_bind: Mat4,
}

impl Bone {
    pub fn name_str(&self) -> &str {
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(32);
        std::str::from_utf8(&self.name[..len]).unwrap_or("")
    }
}

/// A skeleton is a hierarchy of bones with inverse bind matrices,
/// used for GPU skinning and animation.
#[derive(Debug, Clone, PartialEq)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

impl Skeleton {
    pub fn new(bones: Vec<Bone>) -> Self {
        Self { bones }
    }

    pub fn bone_count(&self) -> usize {
        self.bones.len()
    }

    pub fn find_bone_index(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name_str() == name)
    }

    /// Compute the world-space transform for each bone from local transforms.
    /// Returns a Vec of world matrices in bone-index order.
    pub fn compute_world_matrices(&self) -> Vec<Mat4> {
        let mut matrices = Vec::with_capacity(self.bones.len());
        for (i, bone) in self.bones.iter().enumerate() {
            let local = Mat4::from_scale_rotation_translation(
                bone.local_scl,
                rustix_core::math::Quat::from_euler(
                    rustix_core::math::EulerRot::XYZ,
                    bone.local_rot.x,
                    bone.local_rot.y,
                    bone.local_rot.z,
                ),
                bone.local_pos,
            );
            if bone.parent == u16::MAX {
                matrices.push(local);
            } else {
                let parent = matrices[bone.parent as usize];
                matrices.push(parent * local);
            }
        }
        matrices
    }

    /// Compute the final skinning matrices (world * inverse_bind) for each bone.
    pub fn compute_skinning_matrices(&self) -> Vec<Mat4> {
        let world = self.compute_world_matrices();
        world.into_iter().zip(&self.bones)
            .map(|(w, b)| w * b.inverse_bind)
            .collect()
    }

    /// Retarget local rotations from `source` skeleton to self.
    ///
    /// Bones are matched by name. Target bone `local_pos`, `local_scl`, and
    /// `inverse_bind` are preserved so proportions stay correct for the target.
    /// Call `compute_world_matrices()` afterward to get the retargeted pose.
    pub fn retarget_from(&mut self, source: &Skeleton) {
        for target_bone in &mut self.bones {
            if let Some(source_idx) = source.find_bone_index(target_bone.name_str()) {
                target_bone.local_rot = source.bones[source_idx].local_rot;
            }
        }
    }

    /// Convenience: retarget rotations from `source`, then compute world matrices.
    ///
    /// Returns world-space matrices for self with source rotations but target proportions.
    pub fn retargeted_world_matrices(&mut self, source: &Skeleton) -> Vec<Mat4> {
        self.retarget_from(source);
        self.compute_world_matrices()
    }

    /// Convenience: retarget rotations from `source`, then compute skinning matrices.
    ///
    /// Returns the skinning palette for self with source rotations but target proportions.
    pub fn retargeted_skinning_matrices(&mut self, source: &Skeleton) -> Vec<Mat4> {
        let world = self.retargeted_world_matrices(source);
        world.into_iter().zip(&self.bones)
            .map(|(w, b)| w * b.inverse_bind)
            .collect()
    }
}

impl Skeleton {
    /// Build a runtime `Skeleton` from an asset definition.
    pub fn from_asset(asset: &rustix_asset::skeleton::SkeletonAsset) -> Self {
        let bones = asset.bones.iter().map(|b| Bone {
            name: b.name,
            parent: b.parent,
            local_pos: Vec3::from(b.local_pos),
            local_rot: Vec3::from(b.local_rot),
            local_scl: Vec3::from(b.local_scl),
            inverse_bind: Mat4::from_cols_array_2d(&b.inverse_bind),
        }).collect();
        Self::new(bones)
    }
}
