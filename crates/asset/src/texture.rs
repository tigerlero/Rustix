//! Texture asset format and image importers (PNG, HDR, KTX2 → .rxtex).

use std::future::Future;
use std::pin::Pin;

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer};

// ── Texture Format ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    /// 8-bit RGBA, sRGB-ish. Standard diffuse / color textures.
    R8g8b8a8Unorm = 0,
    /// 16-bit half-float RGBA. HDR / environment maps.
    R16g16b16a16Sfloat = 1,
    /// 32-bit float RGBA. High precision HDR.
    R32g32b32a32Sfloat = 2,
}

impl TextureFormat {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(TextureFormat::R8g8b8a8Unorm),
            1 => Some(TextureFormat::R16g16b16a16Sfloat),
            2 => Some(TextureFormat::R32g32b32a32Sfloat),
            _ => None,
        }
    }

    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            TextureFormat::R8g8b8a8Unorm => 4,
            TextureFormat::R16g16b16a16Sfloat => 8,
            TextureFormat::R32g32b32a32Sfloat => 16,
        }
    }
}

// ── Texture Asset ──

/// CPU-side texture data that can be serialized to `.rxtex` and later
/// uploaded to the GPU via `Renderer::create_texture_with_format`.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureAsset {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub pixels: Vec<u8>,
    pub mip_levels: u32,
}

impl TextureAsset {
    pub fn new(width: u32, height: u32, format: TextureFormat, pixels: Vec<u8>) -> Self {
        let expected = (width * height) as usize * format.bytes_per_pixel();
        assert_eq!(
            pixels.len(), expected,
            "pixel buffer size mismatch: got {}, expected {}", pixels.len(), expected
        );
        Self { width, height, format, pixels, mip_levels: 1 }
    }

    pub fn with_mips(mut self, levels: u32) -> Self {
        self.mip_levels = levels;
        self
    }
}

impl Asset for TextureAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::TextureAsset")
    }
}

// ── .rxtex binary format ──

const RXTEX_MAGIC: &[u8; 4] = b"RXT1";
const RXTEX_VERSION: u32 = 1;

pub fn import_rxtex(bytes: &[u8]) -> ImportResult<TextureAsset> {
    if bytes.len() < 24 {
        return Err("rxtex: file too small for header".to_string());
    }

    if &bytes[0..4] != RXTEX_MAGIC {
        return Err("rxtex: invalid magic".to_string());
    }

    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXTEX_VERSION {
        return Err(format!("rxtex: unsupported version {version}"));
    }

    let width = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let height = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let format_raw = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let mip_levels = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);

    let format = TextureFormat::from_u32(format_raw)
        .ok_or_else(|| format!("rxtex: unknown format {format_raw}"))?;

    let data_size = (width * height) as usize * format.bytes_per_pixel();
    let header_size = 24;
    if bytes.len() < header_size + data_size {
        return Err("rxtex: file too small for pixel data".to_string());
    }

    Ok(TextureAsset {
        width,
        height,
        format,
        pixels: bytes[header_size..header_size + data_size].to_vec(),
        mip_levels,
    })
}

pub fn export_rxtex(asset: &TextureAsset) -> Vec<u8> {
    let data_size = asset.pixels.len();
    let total = 24 + data_size;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(RXTEX_MAGIC);
    out.extend_from_slice(&RXTEX_VERSION.to_le_bytes());
    out.extend_from_slice(&asset.width.to_le_bytes());
    out.extend_from_slice(&asset.height.to_le_bytes());
    out.extend_from_slice(&(asset.format as u32).to_le_bytes());
    out.extend_from_slice(&asset.mip_levels.to_le_bytes());
    out.extend_from_slice(&asset.pixels);
    out
}

// ── PNG Importer ──

pub struct PngTextureImporter;

impl Importer for PngTextureImporter {
    type Asset = TextureAsset;

    fn name(&self) -> &'static str { "png_texture" }
    fn extensions(&self) -> &[&'static str] { &["png"] }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_png(bytes)))
    }
}

pub fn import_png(bytes: &[u8]) -> ImportResult<TextureAsset> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("PNG decode: {e}"))?;
    let rgba = img.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    Ok(TextureAsset::new(width, height, TextureFormat::R8g8b8a8Unorm, rgba.into_raw()))
}

// ── HDR Importer ──

pub struct HdrTextureImporter;

impl Importer for HdrTextureImporter {
    type Asset = TextureAsset;

    fn name(&self) -> &'static str { "hdr_texture" }
    fn extensions(&self) -> &[&'static str] { &["hdr"] }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_hdr(bytes)))
    }
}

fn import_hdr(bytes: &[u8]) -> ImportResult<TextureAsset> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("HDR decode: {e}"))?;
    // Convert to RGBA32F then pack into half-floats
    let rgba = img.to_rgba32f();
    let (width, height) = (rgba.width(), rgba.height());
    let mut pixels = Vec::with_capacity((width * height * 8) as usize);
    for pix in rgba.pixels() {
        for ch in pix.0.iter() {
            let h = half::f16::from_f32(*ch);
            pixels.extend_from_slice(&h.to_le_bytes());
        }
    }
    Ok(TextureAsset::new(width, height, TextureFormat::R16g16b16a16Sfloat, pixels))
}

// ── KTX2 Importer ──

pub struct Ktx2TextureImporter;

impl Importer for Ktx2TextureImporter {
    type Asset = TextureAsset;

    fn name(&self) -> &'static str { "ktx2_texture" }
    fn extensions(&self) -> &[&'static str] { &["ktx2"] }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_ktx2(bytes)))
    }
}

fn import_ktx2(bytes: &[u8]) -> ImportResult<TextureAsset> {
    let reader = ktx2::Reader::new(bytes)
        .map_err(|e| format!("KTX2 parse: {e:?}"))?;

    let header = reader.header();
    let width = header.pixel_width;
    let height = header.pixel_height.max(1);
    let mip_levels = header.level_count.max(1);

    // Determine format from KTX2 vk_format
    let (format, bpp) = match header.format {
        Some(ktx2::Format::R8G8B8A8_UNORM) => (TextureFormat::R8g8b8a8Unorm, 4),
        Some(ktx2::Format::R16G16B16A16_SFLOAT) => (TextureFormat::R16g16b16a16Sfloat, 8),
        Some(ktx2::Format::R32G32B32A32_SFLOAT) => (TextureFormat::R32g32b32a32Sfloat, 16),
        Some(ktx2::Format::R8G8B8_UNORM) => {
            // Expand RGB8 to RGBA8
            let mut all_pixels = Vec::new();
            for data in reader.levels() {
                for chunk in data.chunks_exact(3) {
                    all_pixels.push(chunk[0]);
                    all_pixels.push(chunk[1]);
                    all_pixels.push(chunk[2]);
                    all_pixels.push(255);
                }
            }
            return Ok(TextureAsset::new(width, height, TextureFormat::R8g8b8a8Unorm, all_pixels).with_mips(mip_levels));
        }
        Some(other) => {
            tracing::warn!("KTX2 format {:?} not directly supported, attempting RGBA8 fallback", other);
            return Err(format!("KTX2 unsupported format: {:?}", other));
        }
        None => {
            // No VkFormat — try to infer from type-size
            match header.type_size {
                1 => (TextureFormat::R8g8b8a8Unorm, 4),
                2 => (TextureFormat::R16g16b16a16Sfloat, 8),
                4 => (TextureFormat::R32g32b32a32Sfloat, 16),
                _ => return Err(format!("KTX2 unknown type_size {}", header.type_size)),
            }
        }
    };

    let mut all_pixels = Vec::with_capacity((width * height) as usize * bpp);
    for data in reader.levels() {
        all_pixels.extend_from_slice(data);
    }

    Ok(TextureAsset::new(width, height, format, all_pixels).with_mips(mip_levels))
}
