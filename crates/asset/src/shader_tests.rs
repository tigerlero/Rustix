//! Tests for shader asset types and binary format.

use crate::shader::{ShaderAsset, ShaderStage, ShaderLanguage, import_rxshader, export_rxshader};

#[test]
fn shader_stage_to_from_u32() {
    assert_eq!(ShaderStage::Vertex.to_u32(), 0);
    assert_eq!(ShaderStage::Fragment.to_u32(), 1);
    assert_eq!(ShaderStage::Compute.to_u32(), 2);
    assert_eq!(ShaderStage::from_u32(0), Some(ShaderStage::Vertex));
    assert_eq!(ShaderStage::from_u32(1), Some(ShaderStage::Fragment));
    assert_eq!(ShaderStage::from_u32(99), None);
}

#[test]
fn shader_language_to_from_u32() {
    assert_eq!(ShaderLanguage::Glsl.to_u32(), 0);
    assert_eq!(ShaderLanguage::Wgsl.to_u32(), 1);
    assert_eq!(ShaderLanguage::Spv.to_u32(), 2);
    assert_eq!(ShaderLanguage::from_u32(0), Some(ShaderLanguage::Glsl));
    assert_eq!(ShaderLanguage::from_u32(1), Some(ShaderLanguage::Wgsl));
    assert_eq!(ShaderLanguage::from_u32(99), None);
}

#[test]
fn shader_asset_new() {
    let asset = ShaderAsset::new(
        ShaderStage::Vertex,
        ShaderLanguage::Glsl,
        "void main() {}".to_string(),
        vec![0xDEADBEEF],
    );
    assert_eq!(asset.stage, ShaderStage::Vertex);
    assert_eq!(asset.language, ShaderLanguage::Glsl);
    assert_eq!(asset.source, "void main() {}");
    assert_eq!(asset.entry_point, "main");
    assert!(asset.has_compiled_spv());
}

#[test]
fn shader_asset_with_entry_point() {
    let asset = ShaderAsset::new(
        ShaderStage::Compute,
        ShaderLanguage::Wgsl,
        "@compute".to_string(),
        vec![],
    ).with_entry_point("cs_main");
    assert_eq!(asset.entry_point, "cs_main");
    assert!(!asset.has_compiled_spv());
}

#[test]
fn rxshader_roundtrip() {
    let original = ShaderAsset {
        stage: ShaderStage::Fragment,
        language: ShaderLanguage::Glsl,
        source: "void main() { gl_FragColor = vec4(1.0); }".to_string(),
        compiled_spv: vec![0x12345678, 0xABCDEF01],
        entry_point: "main".to_string(),
    };
    let bytes = export_rxshader(&original);
    let imported = import_rxshader(&bytes).unwrap();
    assert_eq!(imported.stage, original.stage);
    assert_eq!(imported.language, original.language);
    assert_eq!(imported.source, original.source);
    assert_eq!(imported.compiled_spv, original.compiled_spv);
    assert_eq!(imported.entry_point, original.entry_point);
}

#[test]
fn rxshader_roundtrip_empty_spv() {
    let original = ShaderAsset {
        stage: ShaderStage::Vertex,
        language: ShaderLanguage::Wgsl,
        source: "@vertex".to_string(),
        compiled_spv: vec![],
        entry_point: "vs_main".to_string(),
    };
    let bytes = export_rxshader(&original);
    let imported = import_rxshader(&bytes).unwrap();
    assert_eq!(imported.compiled_spv, Vec::<u32>::new());
    assert_eq!(imported.entry_point, "vs_main");
}

#[test]
fn rxshader_invalid_magic() {
    let result = import_rxshader(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxshader_too_small() {
    let result = import_rxshader(b"RXS1");
    assert!(result.is_err());
}

#[test]
fn rxshader_unknown_stage() {
    let mut bytes = b"RXS1".to_vec();
    bytes.extend_from_slice(&1u32.to_le_bytes()); // version
    bytes.extend_from_slice(&99u32.to_le_bytes()); // invalid stage
    bytes.extend_from_slice(&0u32.to_le_bytes()); // language
    let result = import_rxshader(&bytes);
    assert!(result.is_err());
}
