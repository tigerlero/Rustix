//! Skeleton asset format and importer (glTF skins → .rxskel).
//!
//! `.rxskel` stores a bone hierarchy with local transforms and inverse bind
//! matrices ready for GPU skinning and animation blending.

use std::future::Future;
use std::pin::Pin;

use rustix_core::math::{Vec3, Mat4};

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer};

// ── Bone Asset ──

/// A single bone in a skeleton hierarchy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneAsset {
    /// Bone name (for debugging and animation targeting).
    pub name: [u8; 32],
    /// Parent bone index, or `u16::MAX` for the root.
    pub parent: u16,
    /// Local position relative to parent.
    pub local_pos: [f32; 3],
    /// Local rotation (Euler angles XYZ).
    pub local_rot: [f32; 3],
    /// Local scale.
    pub local_scl: [f32; 3],
    /// Inverse bind matrix (transforms from model space to bone-local space).
    pub inverse_bind: [[f32; 4]; 4],
}

impl BoneAsset {
    pub fn new(name: &str, parent: u16) -> Self {
        let mut name_bytes = [0u8; 32];
        let bytes = name.as_bytes();
        let len = bytes.len().min(32);
        name_bytes[..len].copy_from_slice(&bytes[..len]);
        Self {
            name: name_bytes,
            parent,
            local_pos: [0.0; 3],
            local_rot: [0.0; 3],
            local_scl: [1.0; 3],
            inverse_bind: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
}

// ── Skeleton Asset ──

/// CPU-side skeleton data that can be serialized to `.rxskel` and later
/// used for skinning and animation retargeting.
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletonAsset {
    pub bones: Vec<BoneAsset>,
}

impl SkeletonAsset {
    pub fn new(bones: Vec<BoneAsset>) -> Self {
        Self { bones }
    }

    pub fn bone_count(&self) -> usize {
        self.bones.len()
    }

    pub fn find_bone_index(&self, name: &str) -> Option<usize> {
        let name_bytes = name.as_bytes();
        self.bones.iter().position(|b| {
            let len = name_bytes.len().min(32);
            let bone_str = std::str::from_utf8(&b.name[..len]).unwrap_or("");
            bone_str == name
        })
    }
}

impl Asset for SkeletonAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::SkeletonAsset")
    }
}

// ── .rxskel binary format ──

const RXSKEL_MAGIC: &[u8; 4] = b"RXK1";
const RXSKEL_VERSION: u32 = 1;

pub fn import_rxskel(bytes: &[u8]) -> ImportResult<SkeletonAsset> {
    if bytes.len() < 8 {
        return Err("rxskel: file too small for header".to_string());
    }
    if &bytes[0..4] != RXSKEL_MAGIC {
        return Err("rxskel: invalid magic".to_string());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXSKEL_VERSION {
        return Err(format!("rxskel: unsupported version {version}"));
    }

    let mut offset = 8usize;
    if bytes.len() < offset + 4 {
        return Err("rxskel: truncated bone count".to_string());
    }
    let bone_count = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]) as usize;
    offset += 4;

    let bone_size = 32 + 2 + 4 * 3 + 4 * 3 + 4 * 3 + 4 * 16; // name + parent + pos + rot + scl + inv_bind
    if bytes.len() < offset + bone_count * bone_size {
        return Err("rxskel: truncated bone data".to_string());
    }

    let mut bones = Vec::with_capacity(bone_count);
    for _ in 0..bone_count {
        let mut name = [0u8; 32];
        name.copy_from_slice(&bytes[offset..offset + 32]);
        offset += 32;

        let parent = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;

        let local_pos = [
            f32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]),
            f32::from_le_bytes([bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7]]),
            f32::from_le_bytes([bytes[offset + 8], bytes[offset + 9], bytes[offset + 10], bytes[offset + 11]]),
        ];
        offset += 12;

        let local_rot = [
            f32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]),
            f32::from_le_bytes([bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7]]),
            f32::from_le_bytes([bytes[offset + 8], bytes[offset + 9], bytes[offset + 10], bytes[offset + 11]]),
        ];
        offset += 12;

        let local_scl = [
            f32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]),
            f32::from_le_bytes([bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7]]),
            f32::from_le_bytes([bytes[offset + 8], bytes[offset + 9], bytes[offset + 10], bytes[offset + 11]]),
        ];
        offset += 12;

        let mut inverse_bind = [[0.0f32; 4]; 4];
        for row in 0..4 {
            for col in 0..4 {
                let idx = offset + (row * 4 + col) * 4;
                inverse_bind[row][col] = f32::from_le_bytes([
                    bytes[idx], bytes[idx + 1], bytes[idx + 2], bytes[idx + 3],
                ]);
            }
        }
        offset += 64;

        bones.push(BoneAsset {
            name,
            parent,
            local_pos,
            local_rot,
            local_scl,
            inverse_bind,
        });
    }

    Ok(SkeletonAsset::new(bones))
}

pub fn export_rxskel(asset: &SkeletonAsset) -> Vec<u8> {
    let bone_size = 32 + 2 + 12 + 12 + 12 + 64; // 134 bytes per bone
    let total = 8 + 4 + asset.bones.len() * bone_size;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(RXSKEL_MAGIC);
    out.extend_from_slice(&RXSKEL_VERSION.to_le_bytes());
    out.extend_from_slice(&(asset.bones.len() as u32).to_le_bytes());

    for bone in &asset.bones {
        out.extend_from_slice(&bone.name);
        out.extend_from_slice(&bone.parent.to_le_bytes());
        out.extend_from_slice(&bone.local_pos[0].to_le_bytes());
        out.extend_from_slice(&bone.local_pos[1].to_le_bytes());
        out.extend_from_slice(&bone.local_pos[2].to_le_bytes());
        out.extend_from_slice(&bone.local_rot[0].to_le_bytes());
        out.extend_from_slice(&bone.local_rot[1].to_le_bytes());
        out.extend_from_slice(&bone.local_rot[2].to_le_bytes());
        out.extend_from_slice(&bone.local_scl[0].to_le_bytes());
        out.extend_from_slice(&bone.local_scl[1].to_le_bytes());
        out.extend_from_slice(&bone.local_scl[2].to_le_bytes());
        for row in 0..4 {
            for col in 0..4 {
                out.extend_from_slice(&bone.inverse_bind[row][col].to_le_bytes());
            }
        }
    }

    out
}

// ── glTF Skin Importer ──

pub struct GltfSkeletonImporter;

impl Importer for GltfSkeletonImporter {
    type Asset = SkeletonAsset;

    fn name(&self) -> &'static str {
        "gltf_skeleton"
    }

    fn extensions(&self) -> &[&'static str] {
        &["gltf", "glb"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_gltf_skins(bytes)))
    }
}

fn import_gltf_skins(bytes: &[u8]) -> ImportResult<SkeletonAsset> {
    let (doc, buffers, _images) = gltf::import_slice(bytes)
        .map_err(|e| format!("glTF parse: {e}"))?;

    let mut all_bones: Vec<BoneAsset> = Vec::new();

    // Build a node index -> parent index map from all scenes
    let mut node_parent: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for node in doc.nodes() {
        for child in node.children() {
            node_parent.insert(child.index(), node.index());
        }
    }

    for skin in doc.skins() {
        let joints: Vec<gltf::Node> = skin.joints().collect();
        if joints.is_empty() {
            continue;
        }

        // Read inverse bind matrices if present
        let ibm_accessor = skin.reader(|buf| Some(&buffers[buf.index()])).read_inverse_bind_matrices();
        let mut ibms: Vec<[[f32; 4]; 4]> = Vec::new();
        if let Some(iter) = ibm_accessor {
            for m in iter {
                ibms.push(m);
            }
        }

        // Collect joint node indices for quick membership test
        let joint_indices: std::collections::HashSet<usize> = joints.iter().map(|j| j.index()).collect();

        // Map glTF node index to our bone index
        let mut node_to_bone: std::collections::HashMap<usize, u16> = std::collections::HashMap::new();

        for (idx, joint) in joints.iter().enumerate() {
            let bone_idx = all_bones.len() as u16;
            node_to_bone.insert(joint.index(), bone_idx);

            let name = joint.name().unwrap_or("bone");
            let mut bone = BoneAsset::new(name, u16::MAX);

            // Find parent: if the joint's parent is also in the skin joints
            if let Some(&parent_node) = node_parent.get(&joint.index()) {
                if let Some(&parent_idx) = node_to_bone.get(&parent_node) {
                    if joint_indices.contains(&parent_node) {
                        bone.parent = parent_idx;
                    }
                }
            }

            // Extract local transform
            let (t, r, s) = joint.transform().decomposed();
            bone.local_pos = [t[0], t[1], t[2]];
            // Convert quaternion to Euler XYZ
            let q = rustix_core::math::Quat::from_xyzw(r[0], r[1], r[2], r[3]);
            let (ex, ey, ez) = q.to_euler(rustix_core::math::EulerRot::XYZ);
            bone.local_rot = [ex, ey, ez];
            bone.local_scl = [s[0], s[1], s[2]];

            // Set inverse bind matrix
            if idx < ibms.len() {
                bone.inverse_bind = ibms[idx];
            }

            all_bones.push(bone);
        }
    }

    if all_bones.is_empty() {
        return Err("glTF file contains no skin data".to_string());
    }

    Ok(SkeletonAsset::new(all_bones))
}

/// Importer for the native `.rxskel` binary format.
pub struct RxskelImporter;

impl Importer for RxskelImporter {
    type Asset = SkeletonAsset;

    fn name(&self) -> &'static str {
        "rxskel"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rxskel"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_rxskel(bytes)))
    }
}
