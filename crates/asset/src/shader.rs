//! Shader asset format and importers (GLSL / WGSL / SPIR-V → .rxshader).
//!
//! `.rxshader` stores both the original source code (for hot-reload and debugging)
//! and the compiled SPIR-V binary (for fast GPU upload). Standard shader stages
//! (vertex, fragment, compute) are compiled via naga at import time. Mesh/task
//! shaders and raw `.spv` files store SPIR-V directly or leave compilation to the
//! renderer.

use std::future::Future;
use std::pin::Pin;

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer};

// ── Enums ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShaderStage {
    Vertex = 0,
    Fragment = 1,
    Compute = 2,
    Mesh = 3,
    Task = 4,
}

impl ShaderStage {
    pub fn to_u32(&self) -> u32 {
        *self as u32
    }

    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(ShaderStage::Vertex),
            1 => Some(ShaderStage::Fragment),
            2 => Some(ShaderStage::Compute),
            3 => Some(ShaderStage::Mesh),
            4 => Some(ShaderStage::Task),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShaderLanguage {
    Glsl = 0,
    Wgsl = 1,
    Spv = 2,
}

impl ShaderLanguage {
    pub fn to_u32(&self) -> u32 {
        *self as u32
    }

    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(ShaderLanguage::Glsl),
            1 => Some(ShaderLanguage::Wgsl),
            2 => Some(ShaderLanguage:: Spv),
            _ => None,
        }
    }
}

// ── Shader Asset ──

/// CPU-side shader data that can be serialized to `.rxshader` and later
/// uploaded to the GPU via `ShaderModule::from_spirv` or recompiled from
/// the stored source.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShaderAsset {
    pub stage: ShaderStage,
    pub language: ShaderLanguage,
    pub source: String,
    pub compiled_spv: Vec<u32>,
    pub entry_point: String,
}

impl ShaderAsset {
    pub fn new(stage: ShaderStage, language: ShaderLanguage, source: String, compiled_spv: Vec<u32>) -> Self {
        Self {
            stage,
            language,
            source,
            compiled_spv,
            entry_point: "main".to_string(),
        }
    }

    pub fn with_entry_point(mut self, ep: impl Into<String>) -> Self {
        self.entry_point = ep.into();
        self
    }

    pub fn has_compiled_spv(&self) -> bool {
        !self.compiled_spv.is_empty()
    }
}

impl Asset for ShaderAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::ShaderAsset")
    }
}

// ── .rxshader binary format ──

const RXSHADER_MAGIC: &[u8; 4] = b"RXS1";
const RXSHADER_VERSION: u32 = 1;

fn write_string(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn read_string(bytes: &[u8], offset: &mut usize) -> ImportResult<String> {
    if bytes.len() < *offset + 4 {
        return Err("rxshader: truncated string length".to_string());
    }
    let len = u32::from_le_bytes([bytes[*offset], bytes[*offset + 1], bytes[*offset + 2], bytes[*offset + 3]]) as usize;
    *offset += 4;
    if bytes.len() < *offset + len {
        return Err("rxshader: truncated string data".to_string());
    }
    let s = String::from_utf8(bytes[*offset..*offset + len].to_vec())
        .map_err(|_| "rxshader: invalid utf-8 in string".to_string())?;
    *offset += len;
    Ok(s)
}

pub fn import_rxshader(bytes: &[u8]) -> ImportResult<ShaderAsset> {
    if bytes.len() < 16 {
        return Err("rxshader: file too small for header".to_string());
    }
    if &bytes[0..4] != RXSHADER_MAGIC {
        return Err("rxshader: invalid magic".to_string());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXSHADER_VERSION {
        return Err(format!("rxshader: unsupported version {version}"));
    }

    let stage_raw = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let lang_raw = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

    let stage = ShaderStage::from_u32(stage_raw)
        .ok_or_else(|| format!("rxshader: unknown stage {stage_raw}"))?;
    let language = ShaderLanguage::from_u32(lang_raw)
        .ok_or_else(|| format!("rxshader: unknown language {lang_raw}"))?;

    let mut offset = 16usize;
    let entry_point = read_string(bytes, &mut offset)?;
    let source = read_string(bytes, &mut offset)?;

    if bytes.len() < offset + 4 {
        return Err("rxshader: truncated spv count".to_string());
    }
    let spv_count = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]) as usize;
    offset += 4;
    if bytes.len() < offset + spv_count * 4 {
        return Err("rxshader: truncated spv data".to_string());
    }
    let compiled_spv: Vec<u32> = bytes[offset..offset + spv_count * 4]
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    Ok(ShaderAsset {
        stage,
        language,
        source,
        compiled_spv,
        entry_point,
    })
}

pub fn export_rxshader(asset: &ShaderAsset) -> Vec<u8> {
    let total = 16
        + 4 + asset.entry_point.len()
        + 4 + asset.source.len()
        + 4 + asset.compiled_spv.len() * 4;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(RXSHADER_MAGIC);
    out.extend_from_slice(&RXSHADER_VERSION.to_le_bytes());
    out.extend_from_slice(&(asset.stage.to_u32()).to_le_bytes());
    out.extend_from_slice(&(asset.language.to_u32()).to_le_bytes());
    write_string(&mut out, &asset.entry_point);
    write_string(&mut out, &asset.source);
    out.extend_from_slice(&(asset.compiled_spv.len() as u32).to_le_bytes());
    for word in &asset.compiled_spv {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out
}

// ── Compilation helpers ──

fn spv_options() -> naga::back::spv::Options<'static> {
    naga::back::spv::Options {
        flags: naga::back::spv::WriterFlags::LABEL_VARYINGS
            | naga::back::spv::WriterFlags::CLAMP_FRAG_DEPTH,
        ..Default::default()
    }
}

fn naga_stage(stage: ShaderStage) -> naga::ShaderStage {
    match stage {
        ShaderStage::Vertex => naga::ShaderStage::Vertex,
        ShaderStage::Fragment => naga::ShaderStage::Fragment,
        ShaderStage::Compute => naga::ShaderStage::Compute,
        ShaderStage::Mesh | ShaderStage::Task => {
            // naga does not support mesh/task; fallback to compute so the parse
            // at least produces something, but the caller should detect this and
            // leave compiled_spv empty.
            naga::ShaderStage::Compute
        }
    }
}

fn compile_glsl(source: &str, stage: naga::ShaderStage) -> Result<Vec<u32>, String> {
    let mut fe = naga::front::glsl::Frontend::default();
    let m = fe
        .parse(&naga::front::glsl::Options::from(stage), source)
        .map_err(|e| format!("GLSL parse: {e}"))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&m)
    .map_err(|e| format!("GLSL validate: {e}"))?;
    naga::back::spv::write_vec(&m, &info, &spv_options(), None)
        .map_err(|e| format!("SPIR-V emit: {e}"))
}

fn compile_wgsl(source: &str, _stage: naga::ShaderStage) -> Result<Vec<u32>, String> {
    let m = naga::front::wgsl::parse_str(source)
        .map_err(|e| format!("WGSL parse: {e}"))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&m)
    .map_err(|e| format!("WGSL validate: {e}"))?;
    naga::back::spv::write_vec(&m, &info, &spv_options(), None)
        .map_err(|e| format!("SPIR-V emit: {e}"))
}

// ── Importers ──

fn infer_stage(ext: &str) -> Option<ShaderStage> {
    match ext {
        "vert" => Some(ShaderStage::Vertex),
        "frag" => Some(ShaderStage::Fragment),
        "comp" => Some(ShaderStage::Compute),
        "mesh" => Some(ShaderStage::Mesh),
        "task" => Some(ShaderStage::Task),
        _ => None,
    }
}

/// Importer for GLSL source files (.glsl, .vert, .frag, .comp, .mesh, .task).
pub struct GlslShaderImporter;

impl Importer for GlslShaderImporter {
    type Asset = ShaderAsset;

    fn name(&self) -> &'static str {
        "glsl_shader"
    }

    fn extensions(&self) -> &[&'static str] {
        &["glsl", "vert", "frag", "comp", "mesh", "task"]
    }

    fn import<'a>(&self, bytes: &'a [u8], hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_glsl(bytes, hint)))
    }
}

fn import_glsl(bytes: &[u8], hint: Option<&str>) -> ImportResult<ShaderAsset> {
    let source = std::str::from_utf8(bytes)
        .map_err(|e| format!("invalid utf-8: {e}"))?;

    let ext = hint
        .and_then(|p| std::path::Path::new(p).extension())
        .and_then(|e| e.to_str())
        .unwrap_or("glsl");

    let stage = infer_stage(ext)
        .ok_or_else(|| format!("cannot infer shader stage from extension '{ext}'"))?;

    // Mesh/task are not supported by naga; leave compiled_spv empty.
    let compiled_spv = if matches!(stage, ShaderStage::Mesh | ShaderStage::Task) {
        Vec::new()
    } else {
        compile_glsl(source, naga_stage(stage))?
    };

    Ok(ShaderAsset::new(stage, ShaderLanguage::Glsl, source.to_string(), compiled_spv))
}

/// Importer for WGSL source files (.wgsl).
pub struct WgslShaderImporter;

impl Importer for WgslShaderImporter {
    type Asset = ShaderAsset;

    fn name(&self) -> &'static str {
        "wgsl_shader"
    }

    fn extensions(&self) -> &[&'static str] {
        &["wgsl"]
    }

    fn import<'a>(&self, bytes: &'a [u8], hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_wgsl(bytes, hint)))
    }
}

fn import_wgsl(bytes: &[u8], hint: Option<&str>) -> ImportResult<ShaderAsset> {
    let source = std::str::from_utf8(bytes)
        .map_err(|e| format!("invalid utf-8: {e}"))?;

    let ext = hint
        .and_then(|p| std::path::Path::new(p).extension())
        .and_then(|e| e.to_str())
        .unwrap_or("wgsl");

    // WGSL does not encode stage in the file; default to vertex if we can't tell,
    // but the shader source itself should declare the stage.
    let stage = infer_stage(ext).unwrap_or(ShaderStage::Vertex);

    let compiled_spv = compile_wgsl(source, naga_stage(stage))?;

    Ok(ShaderAsset::new(stage, ShaderLanguage::Wgsl, source.to_string(), compiled_spv))
}

/// Importer for pre-compiled SPIR-V binary files (.spv).
pub struct SpvShaderImporter;

impl Importer for SpvShaderImporter {
    type Asset = ShaderAsset;

    fn name(&self) -> &'static str {
        "spv_shader"
    }

    fn extensions(&self) -> &[&'static str] {
        &["spv"]
    }

    fn import<'a>(&self, bytes: &'a [u8], hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_spv(bytes, hint)))
    }
}

fn import_spv(bytes: &[u8], _hint: Option<&str>) -> ImportResult<ShaderAsset> {
    if bytes.len() % 4 != 0 {
        return Err("spv: byte length is not a multiple of 4".to_string());
    }
    if bytes.len() < 20 {
        return Err("spv: file too small for SPIR-V header".to_string());
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != 0x07230203 {
        return Err(format!("spv: invalid SPIR-V magic {magic:#08x}"));
    }
    let compiled_spv: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    // SPIR-V does not carry stage info in the binary (for single-shader modules).
    // Default to vertex; the caller must override if known.
    let source = format!("// SPIR-V binary, {} words", compiled_spv.len());
    Ok(ShaderAsset::new(ShaderStage::Vertex, ShaderLanguage::Spv, source, compiled_spv))
}

/// Importer for the native `.rxshader` binary format.
pub struct RxshaderImporter;

impl Importer for RxshaderImporter {
    type Asset = ShaderAsset;

    fn name(&self) -> &'static str {
        "rxshader"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rxshader"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_rxshader(bytes)))
    }
}
