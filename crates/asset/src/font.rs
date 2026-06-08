//! Font asset format and importer (.rxfont).
//!
//! `.rxfont` stores raw TrueType / OpenType font data (`.ttf` / `.otf` bytes)
//! so the engine's UI text rendering (`fontdue` glyph atlas) can load fonts
//! through the asset pipeline instead of `include_bytes!`.

use std::future::Future;
use std::pin::Pin;

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer};

// ── Font Asset ──

/// CPU-side font data that can be serialized to `.rxfont` and later
/// loaded into a glyph atlas for UI text rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct FontAsset {
    /// Human-readable font name (usually the file stem).
    pub name: String,
    /// Raw `.ttf` / `.otf` font file bytes.
    pub data: Vec<u8>,
}

impl FontAsset {
    pub fn new(name: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }
}

impl Asset for FontAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::FontAsset")
    }
}

// ── .rxfont binary format ──

const RXFONT_MAGIC: &[u8; 4] = b"RXF1";
const RXFONT_VERSION: u32 = 1;

pub fn import_rxfont(bytes: &[u8]) -> ImportResult<FontAsset> {
    if bytes.len() < 8 {
        return Err("rxfont: file too small for header".to_string());
    }
    if &bytes[0..4] != RXFONT_MAGIC {
        return Err("rxfont: invalid magic".to_string());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXFONT_VERSION {
        return Err(format!("rxfont: unsupported version {version}"));
    }

    if bytes.len() < 12 {
        return Err("rxfont: file too small for name length".to_string());
    }
    let name_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let name_end = 12 + name_len;
    if bytes.len() < name_end + 4 {
        return Err("rxfont: truncated name or missing data length".to_string());
    }
    let name = std::str::from_utf8(&bytes[12..name_end])
        .map_err(|e| format!("rxfont: invalid name utf-8: {e}"))?
        .to_string();

    let data_len = u32::from_le_bytes([bytes[name_end], bytes[name_end + 1], bytes[name_end + 2], bytes[name_end + 3]]) as usize;
    let data_end = name_end + 4 + data_len;
    if bytes.len() < data_end {
        return Err("rxfont: truncated font data".to_string());
    }
    let data = bytes[name_end + 4..data_end].to_vec();

    Ok(FontAsset { name, data })
}

pub fn export_rxfont(asset: &FontAsset) -> Vec<u8> {
    let name_bytes = asset.name.as_bytes();
    let mut out = Vec::with_capacity(12 + name_bytes.len() + 4 + asset.data.len());
    out.extend_from_slice(RXFONT_MAGIC);
    out.extend_from_slice(&RXFONT_VERSION.to_le_bytes());
    out.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(name_bytes);
    out.extend_from_slice(&(asset.data.len() as u32).to_le_bytes());
    out.extend_from_slice(&asset.data);
    out
}

// ── Importers ──

/// Importer for raw `.ttf` and `.otf` font files.
pub struct TtfFontImporter;

impl Importer for TtfFontImporter {
    type Asset = FontAsset;

    fn name(&self) -> &'static str {
        "ttf_font"
    }

    fn extensions(&self) -> &[&'static str] {
        &["ttf", "otf"]
    }

    fn import<'a>(&self, bytes: &'a [u8], hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        let name = hint.map(|s| {
            std::path::Path::new(s)
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("font")
                .to_string()
        }).unwrap_or_else(|| "font".to_string());
        Box::pin(std::future::ready(Ok(FontAsset::new(name, bytes.to_vec()))))
    }
}

/// Importer for the native `.rxfont` binary format.
pub struct RxfontImporter;

impl Importer for RxfontImporter {
    type Asset = FontAsset;

    fn name(&self) -> &'static str {
        "rxfont"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rxfont"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_rxfont(bytes)))
    }
}
