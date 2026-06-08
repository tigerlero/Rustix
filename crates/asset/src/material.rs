//! Material asset format and importer.
//!
//! `.rxmat` is the engine's native material format storing PBR scalar parameters
//! and optional texture path references.

use std::future::Future;
use std::pin::Pin;

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer, SerializableAsset, import_ron, import_json};

// ── Alpha Mode ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}

impl AlphaMode {
    pub fn to_u32(&self) -> u32 {
        match self {
            AlphaMode::Opaque => 0,
            AlphaMode::Mask => 1,
            AlphaMode::Blend => 2,
        }
    }

    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(AlphaMode::Opaque),
            1 => Some(AlphaMode::Mask),
            2 => Some(AlphaMode::Blend),
            _ => None,
        }
    }
}

// ── Texture Slot ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TextureSlot {
    Albedo = 0,
    Normal = 1,
    MetallicRoughness = 2,
    Emissive = 3,
    Occlusion = 4,
}

impl TextureSlot {
    pub fn to_u32(&self) -> u32 {
        *self as u32
    }

    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(TextureSlot::Albedo),
            1 => Some(TextureSlot::Normal),
            2 => Some(TextureSlot::MetallicRoughness),
            3 => Some(TextureSlot::Emissive),
            4 => Some(TextureSlot::Occlusion),
            _ => None,
        }
    }
}

// ── Material Asset ──

/// CPU-side material definition that can be serialized to `.rxmat` and applied
/// to renderable entities.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MaterialAsset {
    pub base_color: [f32; 4],
    pub roughness: f32,
    pub metallic: f32,
    pub ao: f32,
    pub emissive: f32,
    #[serde(default = "default_normal_scale")]
    pub normal_scale: f32,
    #[serde(default = "default_occlusion_strength")]
    pub occlusion_strength: f32,
    #[serde(default = "default_alpha_cutoff")]
    pub alpha_cutoff: f32,
    #[serde(default)]
    pub alpha_mode: AlphaMode,
    #[serde(default)]
    pub albedo_texture: Option<String>,
    #[serde(default)]
    pub normal_texture: Option<String>,
    #[serde(default)]
    pub metallic_roughness_texture: Option<String>,
    #[serde(default)]
    pub emissive_texture: Option<String>,
    #[serde(default)]
    pub occlusion_texture: Option<String>,
}

fn default_normal_scale() -> f32 { 1.0 }
fn default_occlusion_strength() -> f32 { 1.0 }
fn default_alpha_cutoff() -> f32 { 0.5 }

impl Default for MaterialAsset {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            roughness: 0.5,
            metallic: 0.0,
            ao: 1.0,
            emissive: 0.0,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            alpha_cutoff: 0.5,
            alpha_mode: AlphaMode::Opaque,
            albedo_texture: None,
            normal_texture: None,
            metallic_roughness_texture: None,
            emissive_texture: None,
            occlusion_texture: None,
        }
    }
}

impl MaterialAsset {
    /// Return all non-None texture path references in slot order.
    ///
    /// Useful for registering asset dependencies with `AssetServer::declare_dependencies()`.
    pub fn texture_dependencies(&self) -> Vec<&str> {
        let mut deps = Vec::with_capacity(5);
        if let Some(ref p) = self.albedo_texture { deps.push(p.as_str()); }
        if let Some(ref p) = self.normal_texture { deps.push(p.as_str()); }
        if let Some(ref p) = self.metallic_roughness_texture { deps.push(p.as_str()); }
        if let Some(ref p) = self.emissive_texture { deps.push(p.as_str()); }
        if let Some(ref p) = self.occlusion_texture { deps.push(p.as_str()); }
        deps
    }
}

impl Asset for MaterialAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::MaterialAsset")
    }
}

impl SerializableAsset for MaterialAsset {}

// ── .rxmat binary format ──

const RXMAT_MAGIC: &[u8; 4] = b"RXA1";
const RXMAT_VERSION: u32 = 1;
const RXMAT_HEADER_SIZE: usize = 56;

pub fn import_rxmat(bytes: &[u8]) -> ImportResult<MaterialAsset> {
    if bytes.len() < RXMAT_HEADER_SIZE {
        return Err("rxmat: file too small for header".to_string());
    }

    if &bytes[0..4] != RXMAT_MAGIC {
        return Err("rxmat: invalid magic".to_string());
    }

    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXMAT_VERSION {
        return Err(format!("rxmat: unsupported version {version}"));
    }

    let base_color = [
        f32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        f32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        f32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
        f32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
    ];
    let roughness = f32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
    let metallic = f32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]);
    let ao = f32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
    let emissive = f32::from_le_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]);
    let normal_scale = f32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
    let occlusion_strength = f32::from_le_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]);
    let alpha_cutoff = f32::from_le_bytes([bytes[48], bytes[49], bytes[50], bytes[51]]);
    let alpha_mode_raw = u32::from_le_bytes([bytes[52], bytes[53], bytes[54], bytes[55]]);
    let alpha_mode = AlphaMode::from_u32(alpha_mode_raw)
        .ok_or_else(|| format!("rxmat: unknown alpha mode {alpha_mode_raw}"))?;

    let texture_count = u32::from_le_bytes([bytes[56], bytes[57], bytes[58], bytes[59]]) as usize;
    let mut offset = 60usize;

    let mut albedo_texture = None;
    let mut normal_texture = None;
    let mut metallic_roughness_texture = None;
    let mut emissive_texture = None;
    let mut occlusion_texture = None;

    for _ in 0..texture_count {
        if bytes.len() < offset + 8 {
            return Err("rxmat: truncated texture entry".to_string());
        }
        let slot = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]);
        let path_len = u32::from_le_bytes([bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7]]) as usize;
        offset += 8;
        if bytes.len() < offset + path_len {
            return Err("rxmat: truncated texture path".to_string());
        }
        let path = String::from_utf8(bytes[offset..offset + path_len].to_vec())
            .map_err(|_| "rxmat: invalid utf-8 in texture path".to_string())?;
        offset += path_len;

        match TextureSlot::from_u32(slot) {
            Some(TextureSlot::Albedo) => albedo_texture = Some(path),
            Some(TextureSlot::Normal) => normal_texture = Some(path),
            Some(TextureSlot::MetallicRoughness) => metallic_roughness_texture = Some(path),
            Some(TextureSlot::Emissive) => emissive_texture = Some(path),
            Some(TextureSlot::Occlusion) => occlusion_texture = Some(path),
            None => tracing::warn!("rxmat: unknown texture slot {slot}, skipping"),
        }
    }

    Ok(MaterialAsset {
        base_color,
        roughness,
        metallic,
        ao,
        emissive,
        normal_scale,
        occlusion_strength,
        alpha_cutoff,
        alpha_mode,
        albedo_texture,
        normal_texture,
        metallic_roughness_texture,
        emissive_texture,
        occlusion_texture,
    })
}

pub fn export_rxmat(asset: &MaterialAsset) -> Vec<u8> {
    // Compute total size
    let mut textures: Vec<(TextureSlot, &str)> = Vec::with_capacity(5);
    if let Some(ref p) = asset.albedo_texture { textures.push((TextureSlot::Albedo, p.as_str())); }
    if let Some(ref p) = asset.normal_texture { textures.push((TextureSlot::Normal, p.as_str())); }
    if let Some(ref p) = asset.metallic_roughness_texture { textures.push((TextureSlot::MetallicRoughness, p.as_str())); }
    if let Some(ref p) = asset.emissive_texture { textures.push((TextureSlot::Emissive, p.as_str())); }
    if let Some(ref p) = asset.occlusion_texture { textures.push((TextureSlot::Occlusion, p.as_str())); }

    let path_bytes: usize = textures.iter().map(|(_, p)| 8 + p.len()).sum();
    let total = RXMAT_HEADER_SIZE + 4 + path_bytes;

    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(RXMAT_MAGIC);
    out.extend_from_slice(&RXMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&asset.base_color[0].to_le_bytes());
    out.extend_from_slice(&asset.base_color[1].to_le_bytes());
    out.extend_from_slice(&asset.base_color[2].to_le_bytes());
    out.extend_from_slice(&asset.base_color[3].to_le_bytes());
    out.extend_from_slice(&asset.roughness.to_le_bytes());
    out.extend_from_slice(&asset.metallic.to_le_bytes());
    out.extend_from_slice(&asset.ao.to_le_bytes());
    out.extend_from_slice(&asset.emissive.to_le_bytes());
    out.extend_from_slice(&asset.normal_scale.to_le_bytes());
    out.extend_from_slice(&asset.occlusion_strength.to_le_bytes());
    out.extend_from_slice(&asset.alpha_cutoff.to_le_bytes());
    out.extend_from_slice(&(asset.alpha_mode.to_u32()).to_le_bytes());
    out.extend_from_slice(&(textures.len() as u32).to_le_bytes());

    for (slot, path) in &textures {
        out.extend_from_slice(&(slot.to_u32()).to_le_bytes());
        out.extend_from_slice(&(path.len() as u32).to_le_bytes());
        out.extend_from_slice(path.as_bytes());
    }

    out
}

// ── Importers ──

/// Importer for the native `.rxmat` binary format.
pub struct RxmatImporter;

impl Importer for RxmatImporter {
    type Asset = MaterialAsset;

    fn name(&self) -> &'static str { "rxmat" }
    fn extensions(&self) -> &[&'static str] { &["rxmat"] }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_rxmat(bytes)))
    }
}

/// Importer for RON-serialized materials (authoring convenience).
pub struct RonMaterialImporter;

impl Importer for RonMaterialImporter {
    type Asset = MaterialAsset;

    fn name(&self) -> &'static str { "ron_material" }
    fn extensions(&self) -> &[&'static str] { &["mat.ron", "ron"] }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_ron::<MaterialAsset>(bytes)))
    }
}

/// Importer for JSON-serialized materials (authoring convenience).
pub struct JsonMaterialImporter;

impl Importer for JsonMaterialImporter {
    type Asset = MaterialAsset;

    fn name(&self) -> &'static str { "json_material" }
    fn extensions(&self) -> &[&'static str] { &["mat.json", "json"] }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_json::<MaterialAsset>(bytes)))
    }
}

/// Importer that reads PBR material definitions from glTF / GLB files.
pub struct GltfMaterialImporter;

impl Importer for GltfMaterialImporter {
    type Asset = MaterialAsset;

    fn name(&self) -> &'static str { "gltf_material" }
    fn extensions(&self) -> &[&'static str] { &["gltf", "glb"] }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_gltf_material(bytes)))
    }
}

fn import_gltf_material(bytes: &[u8]) -> ImportResult<MaterialAsset> {
    let (doc, buffers, _images) = gltf::import_slice(bytes)
        .map_err(|e| format!("glTF parse: {e}"))?;

    let material = doc.materials().next()
        .ok_or("glTF file contains no materials")?;

    let pbr = material.pbr_metallic_roughness();

    let base_color = pbr.base_color_factor();
    let roughness = pbr.roughness_factor();
    let metallic = pbr.metallic_factor();

    let albedo_texture = pbr.base_color_texture()
        .and_then(|t| image_source_uri(&t.texture()));
    let metallic_roughness_texture = pbr.metallic_roughness_texture()
        .and_then(|t| image_source_uri(&t.texture()));

    let normal_texture = material.normal_texture()
        .and_then(|t| image_source_uri(&t.texture()));
    let normal_scale = material.normal_texture().map(|t| t.scale()).unwrap_or(1.0);

    let emissive_texture = material.emissive_texture()
        .and_then(|t| image_source_uri(&t.texture()));
    let emissive = material.emissive_factor();

    let occlusion_texture = material.occlusion_texture()
        .and_then(|t| image_source_uri(&t.texture()));
    let occlusion_strength = material.occlusion_texture().map(|t| t.strength()).unwrap_or(1.0);

    let alpha_mode = match material.alpha_mode() {
        gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
        gltf::material::AlphaMode::Mask => AlphaMode::Mask,
        gltf::material::AlphaMode::Blend => AlphaMode::Blend,
    };
    let alpha_cutoff = material.alpha_cutoff().unwrap_or(0.5);

    Ok(MaterialAsset {
        base_color: [base_color[0], base_color[1], base_color[2], base_color[3]],
        roughness,
        metallic,
        ao: 1.0,
        emissive: emissive[0] + emissive[1] + emissive[2],
        normal_scale,
        occlusion_strength,
        alpha_cutoff,
        alpha_mode,
        albedo_texture,
        normal_texture,
        metallic_roughness_texture,
        emissive_texture,
        occlusion_texture,
    })
}

fn image_source_uri(texture: &gltf::Texture) -> Option<String> {
    let image = texture.source();
    match image.source() {
        gltf::image::Source::Uri { uri, .. } => Some(uri.to_string()),
        _ => image.name().map(|n| n.to_string()),
    }
}
