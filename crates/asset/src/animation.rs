//! Animation asset format and importer (glTF animations → .rxanim).
//!
//! `.rxanim` stores decoded keyframe tracks ready for engine-side playback,
//! eliminating runtime parsing overhead.

use std::future::Future;
use std::pin::Pin;

use rustix_core::math::{Vec3, Quat};

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer};

// ── Keyframe Asset ──

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyframeAsset {
    pub time: f32,
    pub value: [f32; 3],
}

impl KeyframeAsset {
    pub fn new(time: f32, value: [f32; 3]) -> Self {
        Self { time, value }
    }
}

// ── Animation Clip Asset ──

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationClipAsset {
    pub name: String,
    pub duration: f32,
    pub position_track: Vec<KeyframeAsset>,
    pub rotation_track: Vec<KeyframeAsset>,
    pub scale_track: Vec<KeyframeAsset>,
}

impl AnimationClipAsset {
    pub fn new(name: impl Into<String>, duration: f32) -> Self {
        Self {
            name: name.into(),
            duration,
            position_track: Vec::new(),
            rotation_track: Vec::new(),
            scale_track: Vec::new(),
        }
    }
}

// ── Animation Asset ──

/// CPU-side animation data that can be serialized to `.rxanim` and later
/// applied to entities via the animation system.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationAsset {
    pub clips: Vec<AnimationClipAsset>,
}

impl AnimationAsset {
    pub fn new(clips: Vec<AnimationClipAsset>) -> Self {
        Self { clips }
    }

    pub fn clip_count(&self) -> usize {
        self.clips.len()
    }
}

impl Asset for AnimationAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::AnimationAsset")
    }
}

// ── .rxanim binary format ──

const RXANIM_MAGIC: &[u8; 4] = b"RXN1";
const RXANIM_VERSION: u32 = 1;

fn write_string(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn read_string(bytes: &[u8], offset: &mut usize) -> ImportResult<String> {
    if bytes.len() < *offset + 4 {
        return Err("rxanim: truncated string length".to_string());
    }
    let len = u32::from_le_bytes([bytes[*offset], bytes[*offset + 1], bytes[*offset + 2], bytes[*offset + 3]]) as usize;
    *offset += 4;
    if bytes.len() < *offset + len {
        return Err("rxanim: truncated string data".to_string());
    }
    let s = String::from_utf8(bytes[*offset..*offset + len].to_vec())
        .map_err(|_| "rxanim: invalid utf-8 in string".to_string())?;
    *offset += len;
    Ok(s)
}

fn read_f32(bytes: &[u8], offset: &mut usize) -> ImportResult<f32> {
    if bytes.len() < *offset + 4 {
        return Err("rxanim: truncated f32".to_string());
    }
    let v = f32::from_le_bytes([bytes[*offset], bytes[*offset + 1], bytes[*offset + 2], bytes[*offset + 3]]);
    *offset += 4;
    Ok(v)
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> ImportResult<u32> {
    if bytes.len() < *offset + 4 {
        return Err("rxanim: truncated u32".to_string());
    }
    let v = u32::from_le_bytes([bytes[*offset], bytes[*offset + 1], bytes[*offset + 2], bytes[*offset + 3]]);
    *offset += 4;
    Ok(v)
}

pub fn import_rxanim(bytes: &[u8]) -> ImportResult<AnimationAsset> {
    if bytes.len() < 8 {
        return Err("rxanim: file too small for header".to_string());
    }
    if &bytes[0..4] != RXANIM_MAGIC {
        return Err("rxanim: invalid magic".to_string());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXANIM_VERSION {
        return Err(format!("rxanim: unsupported version {version}"));
    }

    let mut offset = 8usize;
    let clip_count = read_u32(bytes, &mut offset)? as usize;
    let mut clips = Vec::with_capacity(clip_count);

    for _ in 0..clip_count {
        let name = read_string(bytes, &mut offset)?;
        let duration = read_f32(bytes, &mut offset)?;
        let mut clip = AnimationClipAsset::new(name, duration);

        for track in [&mut clip.position_track, &mut clip.rotation_track, &mut clip.scale_track] {
            let count = read_u32(bytes, &mut offset)? as usize;
            track.reserve(count);
            for _ in 0..count {
                let time = read_f32(bytes, &mut offset)?;
                let x = read_f32(bytes, &mut offset)?;
                let y = read_f32(bytes, &mut offset)?;
                let z = read_f32(bytes, &mut offset)?;
                track.push(KeyframeAsset::new(time, [x, y, z]));
            }
        }

        clips.push(clip);
    }

    Ok(AnimationAsset::new(clips))
}

pub fn export_rxanim(asset: &AnimationAsset) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(RXANIM_MAGIC);
    out.extend_from_slice(&RXANIM_VERSION.to_le_bytes());
    out.extend_from_slice(&(asset.clip_count() as u32).to_le_bytes());

    for clip in &asset.clips {
        write_string(&mut out, &clip.name);
        out.extend_from_slice(&clip.duration.to_le_bytes());
        for track in [&clip.position_track, &clip.rotation_track, &clip.scale_track] {
            out.extend_from_slice(&(track.len() as u32).to_le_bytes());
            for kf in track {
                out.extend_from_slice(&kf.time.to_le_bytes());
                out.extend_from_slice(&kf.value[0].to_le_bytes());
                out.extend_from_slice(&kf.value[1].to_le_bytes());
                out.extend_from_slice(&kf.value[2].to_le_bytes());
            }
        }
    }

    out
}

// ── glTF Animation Importer ──

pub struct GltfAnimationImporter;

impl Importer for GltfAnimationImporter {
    type Asset = AnimationAsset;

    fn name(&self) -> &'static str {
        "gltf_animation"
    }

    fn extensions(&self) -> &[&'static str] {
        &["gltf", "glb"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_gltf_animations(bytes)))
    }
}

fn import_gltf_animations(bytes: &[u8]) -> ImportResult<AnimationAsset> {
    let (doc, buffers, _images) = gltf::import_slice(bytes)
        .map_err(|e| format!("glTF parse: {e}"))?;

    let mut clips = Vec::new();

    for anim in doc.animations() {
        let name = anim.name().unwrap_or("unnamed").to_string();
        let mut duration = 0.0f32;

        let mut position_kfs: Vec<KeyframeAsset> = Vec::new();
        let mut rotation_kfs: Vec<KeyframeAsset> = Vec::new();
        let mut scale_kfs: Vec<KeyframeAsset> = Vec::new();

        for channel in anim.channels() {
            let reader = channel.reader(|buf| Some(&buffers[buf.index()]));

            let times: Vec<f32> = if let Some(iter) = reader.read_inputs() {
                iter.collect()
            } else {
                continue;
            };

            if let Some(max_time) = times.last() {
                duration = duration.max(*max_time);
            }

            match channel.target().property() {
                gltf::animation::Property::Translation => {
                    if let Some(outputs) = reader.read_outputs() {
                        if let gltf::animation::util::ReadOutputs::Translations(iter) = outputs {
                            for (t, v) in times.iter().zip(iter) {
                                position_kfs.push(KeyframeAsset::new(*t, v));
                            }
                        }
                    }
                }
                gltf::animation::Property::Rotation => {
                    if let Some(outputs) = reader.read_outputs() {
                        if let gltf::animation::util::ReadOutputs::Rotations(rots) = outputs {
                            let rot_vec: Vec<[f32; 4]> = match rots {
                                gltf::animation::util::Rotations::F32(iter) => {
                                    iter.map(|r| [r[0], r[1], r[2], r[3]]).collect()
                                }
                                gltf::animation::util::Rotations::U8(iter) => {
                                    iter.map(|r| {
                                        let scale = 1.0 / 255.0;
                                        [r[0] as f32 * scale * 2.0 - 1.0, r[1] as f32 * scale * 2.0 - 1.0,
                                         r[2] as f32 * scale * 2.0 - 1.0, r[3] as f32 * scale * 2.0 - 1.0]
                                    }).collect()
                                }
                                gltf::animation::util::Rotations::I8(iter) => {
                                    iter.map(|r| {
                                        let scale = 1.0 / 127.0;
                                        [r[0] as f32 * scale, r[1] as f32 * scale,
                                         r[2] as f32 * scale, r[3] as f32 * scale]
                                    }).collect()
                                }
                                gltf::animation::util::Rotations::U16(iter) => {
                                    iter.map(|r| {
                                        let scale = 1.0 / 65535.0;
                                        [r[0] as f32 * scale * 2.0 - 1.0, r[1] as f32 * scale * 2.0 - 1.0,
                                         r[2] as f32 * scale * 2.0 - 1.0, r[3] as f32 * scale * 2.0 - 1.0]
                                    }).collect()
                                }
                                gltf::animation::util::Rotations::I16(iter) => {
                                    iter.map(|r| {
                                        let scale = 1.0 / 32767.0;
                                        [r[0] as f32 * scale, r[1] as f32 * scale,
                                         r[2] as f32 * scale, r[3] as f32 * scale]
                                    }).collect()
                                }
                            };
                            for (t, rot) in times.iter().zip(rot_vec.iter()) {
                                // glTF rotations are quaternions (x, y, z, w)
                                let q = Quat::from_xyzw(rot[0], rot[1], rot[2], rot[3]);
                                let (x, y, z) = q.to_euler(rustix_core::math::EulerRot::XYZ);
                                rotation_kfs.push(KeyframeAsset::new(*t, [x, y, z]));
                            }
                        }
                    }
                }
                gltf::animation::Property::Scale => {
                    if let Some(outputs) = reader.read_outputs() {
                        if let gltf::animation::util::ReadOutputs::Scales(iter) = outputs {
                            for (t, v) in times.iter().zip(iter) {
                                scale_kfs.push(KeyframeAsset::new(*t, v));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Sort keyframes by time for each track
        position_kfs.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        rotation_kfs.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        scale_kfs.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

        let mut clip = AnimationClipAsset::new(name, duration);
        clip.position_track = position_kfs;
        clip.rotation_track = rotation_kfs;
        clip.scale_track = scale_kfs;
        clips.push(clip);
    }

    if clips.is_empty() {
        return Err("glTF file contains no animation data".to_string());
    }

    Ok(AnimationAsset::new(clips))
}

/// Importer for the native `.rxanim` binary format.
pub struct RxanimImporter;

impl Importer for RxanimImporter {
    type Asset = AnimationAsset;

    fn name(&self) -> &'static str {
        "rxanim"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rxanim"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_rxanim(bytes)))
    }
}
