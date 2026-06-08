//! Prefab asset format and importer (.rxprefab).
//!
//! `.rxprefab` stores a hierarchy of entities with transforms, meshes, materials,
//! lights, physics, scripts, and parent-child relationships. It is serialized
//! as RON inside a binary wrapper (magic + version + length-prefixed RON bytes)
//! so it remains human-editable while being identifiable by the asset pipeline.

use std::future::Future;
use std::pin::Pin;

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer};

// ── Inline component representations (lightweight, no external crate deps) ──

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<[f32; 3]> for PrefabVec3 {
    fn from(v: [f32; 3]) -> Self {
        Self { x: v[0], y: v[1], z: v[2] }
    }
}

impl From<PrefabVec3> for [f32; 3] {
    fn from(v: PrefabVec3) -> Self {
        [v.x, v.y, v.z]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum PrefabBodyType {
    #[default]
    Dynamic,
    Static,
    Kinematic,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabDirectionalLight {
    pub color: PrefabVec3,
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabPointLight {
    pub color: PrefabVec3,
    pub intensity: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabSpotLight {
    pub color: PrefabVec3,
    pub intensity: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabCamera {
    pub fov_degrees: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabAudioListener {
    pub position: PrefabVec3,
    pub forward: PrefabVec3,
    pub up: PrefabVec3,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabAudioSource {
    pub position: PrefabVec3,
    pub min_distance: f32,
    pub max_distance: f32,
    pub rolloff: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabRigidBody {
    pub body_type: PrefabBodyType,
    pub mass: f32,
    pub velocity: PrefabVec3,
    pub angular_velocity: PrefabVec3,
    pub gravity_scale: f32,
    pub drag: f32,
    pub angular_drag: f32,
    pub use_gravity: bool,
    pub can_sleep: bool,
    pub sleeping: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PrefabColliderShape {
    Sphere { radius: f32 },
    Box { half_extents: PrefabVec3 },
    Capsule { radius: f32, height: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabCollider {
    pub shape: PrefabColliderShape,
    pub is_trigger: bool,
    pub restitution: f32,
    pub friction: f32,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabScriptConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabScript {
    pub source: String,
    pub config: PrefabScriptConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabMaterial {
    pub base_color: PrefabVec3,
    pub alpha: f32,
    pub roughness: f32,
    pub metallic: f32,
    pub ao: f32,
    pub emissive: f32,
}

// ── Prefab Entity ──

/// A single entity definition inside a prefab.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabEntity {
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<PrefabMaterial>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirlight: Option<PrefabDirectionalLight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointlight: Option<PrefabPointLight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spotlight: Option<PrefabSpotLight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<PrefabScript>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rigidbody: Option<PrefabRigidBody>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collider: Option<PrefabCollider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audiolistener: Option<PrefabAudioListener>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audiosource: Option<PrefabAudioSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<PrefabCamera>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_idx: Option<usize>,
}

// ── Prefab Data ──

/// The top-level data for a prefab: an ordered list of entities with optional
/// parent indices forming a hierarchy.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrefabData {
    pub entities: Vec<PrefabEntity>,
}

// ── Prefab Asset ──

/// CPU-side prefab data that can be serialized to `.rxprefab` and later
/// instantiated into an ECS world.
#[derive(Debug, Clone, PartialEq)]
pub struct PrefabAsset {
    pub data: PrefabData,
}

impl PrefabAsset {
    pub fn new(data: PrefabData) -> Self {
        Self { data }
    }

    pub fn entity_count(&self) -> usize {
        self.data.entities.len()
    }
}

impl Asset for PrefabAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::PrefabAsset")
    }
}

// ── .rxprefab binary format ──

const RXPREFAB_MAGIC: &[u8; 4] = b"RXP1";
const RXPREFAB_VERSION: u32 = 1;

pub fn import_rxprefab(bytes: &[u8]) -> ImportResult<PrefabAsset> {
    if bytes.len() < 8 {
        return Err("rxprefab: file too small for header".to_string());
    }
    if &bytes[0..4] != RXPREFAB_MAGIC {
        return Err("rxprefab: invalid magic".to_string());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXPREFAB_VERSION {
        return Err(format!("rxprefab: unsupported version {version}"));
    }

    if bytes.len() < 12 {
        return Err("rxprefab: file too small for length".to_string());
    }
    let ron_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if bytes.len() < 12 + ron_len {
        return Err("rxprefab: truncated RON data".to_string());
    }

    let ron_str = std::str::from_utf8(&bytes[12..12 + ron_len])
        .map_err(|e| format!("rxprefab: invalid utf-8: {e}"))?;

    let data: PrefabData = ron::from_str(ron_str)
        .map_err(|e| format!("rxprefab: RON parse error: {e}"))?;

    Ok(PrefabAsset::new(data))
}

pub fn export_rxprefab(asset: &PrefabAsset) -> Vec<u8> {
    let ron_str = ron::ser::to_string_pretty(&asset.data, ron::ser::PrettyConfig::default())
        .expect("PrefabData serializes to RON");
    let ron_bytes = ron_str.as_bytes();
    let mut out = Vec::with_capacity(12 + ron_bytes.len());
    out.extend_from_slice(RXPREFAB_MAGIC);
    out.extend_from_slice(&RXPREFAB_VERSION.to_le_bytes());
    out.extend_from_slice(&(ron_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(ron_bytes);
    out
}

// ── Importers ──

/// Importer for the native `.rxprefab` binary format (RON-wrapped).
pub struct RxprefabImporter;

impl Importer for RxprefabImporter {
    type Asset = PrefabAsset;

    fn name(&self) -> &'static str {
        "rxprefab"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rxprefab"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_rxprefab(bytes)))
    }
}

/// Importer for raw RON prefab files (authoring convenience).
pub struct RonPrefabImporter;

impl Importer for RonPrefabImporter {
    type Asset = PrefabAsset;

    fn name(&self) -> &'static str {
        "ron_prefab"
    }

    fn extensions(&self) -> &[&'static str] {
        &["prefab.ron", "ron"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(async move {
            let s = std::str::from_utf8(bytes).map_err(|e| format!("invalid utf-8: {e}"))?;
            let data: PrefabData = ron::from_str(s).map_err(|e| format!("RON parse error: {e}"))?;
            Ok(PrefabAsset::new(data))
        })
    }
}
