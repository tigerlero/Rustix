//! Region / world asset format and importer (.rxregion).
//!
//! `.rxregion` stores a complete level or zone: region metadata
//! (ambient lighting, sky color) plus a hierarchy of entities.
//! Entity definitions reuse the same inline component structure as
//! prefabs (`PrefabEntity`).

use std::future::Future;
use std::pin::Pin;

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer};

// ── Region Metadata ──

/// Level-wide settings for a region / world asset.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RegionMetadata {
    pub name: String,
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub sky_color: [f32; 3],
    pub fog_color: [f32; 3],
    pub fog_density: f32,
}

impl Default for RegionMetadata {
    fn default() -> Self {
        Self {
            name: "Untitled Region".into(),
            ambient_color: [0.1, 0.1, 0.1],
            ambient_intensity: 0.3,
            sky_color: [0.5, 0.7, 1.0],
            fog_color: [0.5, 0.7, 1.0],
            fog_density: 0.0,
        }
    }
}

// ── Region Data ──

/// The top-level data for a region / world.
///
/// Contains level metadata plus an ordered list of entities with optional
/// parent indices forming a hierarchy. Entity definitions reuse
/// `PrefabEntity` from the prefab module.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RegionData {
    #[serde(default)]
    pub metadata: RegionMetadata,
    pub entities: Vec<crate::prefab::PrefabEntity>,
}

// ── Region Asset ──

/// CPU-side region / world data that can be serialized to `.rxregion`
/// and later instantiated into an ECS world.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionAsset {
    pub data: RegionData,
}

impl RegionAsset {
    pub fn new(data: RegionData) -> Self {
        Self { data }
    }

    pub fn entity_count(&self) -> usize {
        self.data.entities.len()
    }
}

impl Asset for RegionAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::RegionAsset")
    }
}

// ── .rxregion binary format ──

const RXREGION_MAGIC: &[u8; 4] = b"RXR1";
const RXREGION_VERSION: u32 = 1;

pub fn import_rxregion(bytes: &[u8]) -> ImportResult<RegionAsset> {
    if bytes.len() < 8 {
        return Err("rxregion: file too small for header".to_string());
    }
    if &bytes[0..4] != RXREGION_MAGIC {
        return Err("rxregion: invalid magic".to_string());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXREGION_VERSION {
        return Err(format!("rxregion: unsupported version {version}"));
    }

    if bytes.len() < 12 {
        return Err("rxregion: file too small for length".to_string());
    }
    let ron_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if bytes.len() < 12 + ron_len {
        return Err("rxregion: truncated RON data".to_string());
    }

    let ron_str = std::str::from_utf8(&bytes[12..12 + ron_len])
        .map_err(|e| format!("rxregion: invalid utf-8: {e}"))?;

    let data: RegionData = ron::from_str(ron_str)
        .map_err(|e| format!("rxregion: RON parse error: {e}"))?;

    Ok(RegionAsset::new(data))
}

pub fn export_rxregion(asset: &RegionAsset) -> Vec<u8> {
    let ron_str = ron::ser::to_string_pretty(&asset.data, ron::ser::PrettyConfig::default())
        .expect("RegionData serializes to RON");
    let ron_bytes = ron_str.as_bytes();
    let mut out = Vec::with_capacity(12 + ron_bytes.len());
    out.extend_from_slice(RXREGION_MAGIC);
    out.extend_from_slice(&RXREGION_VERSION.to_le_bytes());
    out.extend_from_slice(&(ron_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(ron_bytes);
    out
}

// ── Importers ──

/// Importer for the native `.rxregion` binary format (RON-wrapped).
pub struct RxregionImporter;

impl Importer for RxregionImporter {
    type Asset = RegionAsset;

    fn name(&self) -> &'static str {
        "rxregion"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rxregion"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_rxregion(bytes)))
    }
}

/// Importer for raw RON region files (authoring convenience).
pub struct RonRegionImporter;

impl Importer for RonRegionImporter {
    type Asset = RegionAsset;

    fn name(&self) -> &'static str {
        "ron_region"
    }

    fn extensions(&self) -> &[&'static str] {
        &["region.ron", "ron"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(async move {
            let s = std::str::from_utf8(bytes).map_err(|e| format!("invalid utf-8: {e}"))?;
            let data: RegionData = ron::from_str(s).map_err(|e| format!("RON parse error: {e}"))?;
            Ok(RegionAsset::new(data))
        })
    }
}
