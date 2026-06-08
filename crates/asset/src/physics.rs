//! Physics material asset format and importer (.rxphys).
//!
//! `.rxphys` stores surface physical properties (friction, restitution, density)
//! that can be shared across multiple colliders.

use std::future::Future;
use std::pin::Pin;

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer, SerializableAsset, import_ron, import_json};

// ── Physics Material Asset ──

/// CPU-side physics material data that can be serialized to `.rxphys` and later
/// applied to colliders.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PhysicsMaterialAsset {
    pub static_friction: f32,
    pub dynamic_friction: f32,
    pub restitution: f32,
    pub density: f32,
}

impl Default for PhysicsMaterialAsset {
    fn default() -> Self {
        Self {
            static_friction: 0.5,
            dynamic_friction: 0.5,
            restitution: 0.5,
            density: 1.0,
        }
    }
}

impl Asset for PhysicsMaterialAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::PhysicsMaterialAsset")
    }
}

impl SerializableAsset for PhysicsMaterialAsset {}

// ── .rxphys binary format ──

const RXPHYS_MAGIC: &[u8; 4] = b"RXP1";
const RXPHYS_VERSION: u32 = 1;

pub fn import_rxphys(bytes: &[u8]) -> ImportResult<PhysicsMaterialAsset> {
    if bytes.len() < 24 {
        return Err("rxphys: file too small".to_string());
    }
    if &bytes[0..4] != RXPHYS_MAGIC {
        return Err("rxphys: invalid magic".to_string());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXPHYS_VERSION {
        return Err(format!("rxphys: unsupported version {version}"));
    }

    let static_friction = f32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let dynamic_friction = f32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let restitution = f32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let density = f32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);

    Ok(PhysicsMaterialAsset {
        static_friction,
        dynamic_friction,
        restitution,
        density,
    })
}

pub fn export_rxphys(asset: &PhysicsMaterialAsset) -> Vec<u8> {
    let mut out = Vec::with_capacity(24);
    out.extend_from_slice(RXPHYS_MAGIC);
    out.extend_from_slice(&RXPHYS_VERSION.to_le_bytes());
    out.extend_from_slice(&asset.static_friction.to_le_bytes());
    out.extend_from_slice(&asset.dynamic_friction.to_le_bytes());
    out.extend_from_slice(&asset.restitution.to_le_bytes());
    out.extend_from_slice(&asset.density.to_le_bytes());
    out
}

// ── Importers ──

/// Importer for the native `.rxphys` binary format.
pub struct RxphysImporter;

impl Importer for RxphysImporter {
    type Asset = PhysicsMaterialAsset;

    fn name(&self) -> &'static str {
        "rxphys"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rxphys"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_rxphys(bytes)))
    }
}

/// Importer for RON-serialized physics materials (authoring convenience).
pub struct RonPhysMaterialImporter;

impl Importer for RonPhysMaterialImporter {
    type Asset = PhysicsMaterialAsset;

    fn name(&self) -> &'static str {
        "ron_phys_material"
    }

    fn extensions(&self) -> &[&'static str] {
        &["phys.ron", "ron"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_ron::<PhysicsMaterialAsset>(bytes)))
    }
}

/// Importer for JSON-serialized physics materials (authoring convenience).
pub struct JsonPhysMaterialImporter;

impl Importer for JsonPhysMaterialImporter {
    type Asset = PhysicsMaterialAsset;

    fn name(&self) -> &'static str {
        "json_phys_material"
    }

    fn extensions(&self) -> &[&'static str] {
        &["phys.json", "json"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_json::<PhysicsMaterialAsset>(bytes)))
    }
}
