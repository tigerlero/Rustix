//! Tests for material asset types and binary format.

use crate::material::{MaterialAsset, AlphaMode, TextureSlot, import_rxmat, export_rxmat};

#[test]
fn alpha_mode_default() {
    assert_eq!(AlphaMode::default(), AlphaMode::Opaque);
}

#[test]
fn alpha_mode_to_from_u32() {
    assert_eq!(AlphaMode::Opaque.to_u32(), 0);
    assert_eq!(AlphaMode::Mask.to_u32(), 1);
    assert_eq!(AlphaMode::Blend.to_u32(), 2);
    assert_eq!(AlphaMode::from_u32(0), Some(AlphaMode::Opaque));
    assert_eq!(AlphaMode::from_u32(1), Some(AlphaMode::Mask));
    assert_eq!(AlphaMode::from_u32(2), Some(AlphaMode::Blend));
    assert_eq!(AlphaMode::from_u32(99), None);
}

#[test]
fn texture_slot_to_from_u32() {
    assert_eq!(TextureSlot::Albedo.to_u32(), 0);
    assert_eq!(TextureSlot::Normal.to_u32(), 1);
    assert_eq!(TextureSlot::MetallicRoughness.to_u32(), 2);
    assert_eq!(TextureSlot::Emissive.to_u32(), 3);
    assert_eq!(TextureSlot::Occlusion.to_u32(), 4);
    assert_eq!(TextureSlot::from_u32(0), Some(TextureSlot::Albedo));
    assert_eq!(TextureSlot::from_u32(1), Some(TextureSlot::Normal));
    assert_eq!(TextureSlot::from_u32(5), None);
}

#[test]
fn material_asset_default() {
    let mat = MaterialAsset::default();
    assert_eq!(mat.base_color, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(mat.roughness, 0.5);
    assert_eq!(mat.metallic, 0.0);
    assert_eq!(mat.ao, 1.0);
    assert_eq!(mat.emissive, 0.0);
    assert_eq!(mat.normal_scale, 1.0);
    assert_eq!(mat.occlusion_strength, 1.0);
    assert_eq!(mat.alpha_cutoff, 0.5);
    assert_eq!(mat.alpha_mode, AlphaMode::Opaque);
    assert!(mat.albedo_texture.is_none());
}

#[test]
fn material_asset_texture_dependencies_empty() {
    let mat = MaterialAsset::default();
    assert!(mat.texture_dependencies().is_empty());
}

#[test]
fn material_asset_texture_dependencies_some() {
    let mat = MaterialAsset {
        albedo_texture: Some("albedo.png".to_string()),
        normal_texture: Some("normal.png".to_string()),
        ..Default::default()
    };
    let deps = mat.texture_dependencies();
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0], "albedo.png");
    assert_eq!(deps[1], "normal.png");
}

#[test]
fn material_asset_texture_dependencies_all() {
    let mat = MaterialAsset {
        albedo_texture: Some("a.png".to_string()),
        normal_texture: Some("n.png".to_string()),
        metallic_roughness_texture: Some("mr.png".to_string()),
        emissive_texture: Some("e.png".to_string()),
        occlusion_texture: Some("o.png".to_string()),
        ..Default::default()
    };
    assert_eq!(mat.texture_dependencies().len(), 5);
}

#[test]
fn rxmat_roundtrip() {
    let original = MaterialAsset {
        base_color: [0.5, 0.5, 0.5, 1.0],
        roughness: 0.8,
        metallic: 0.2,
        ao: 0.9,
        emissive: 0.1,
        normal_scale: 0.5,
        occlusion_strength: 0.7,
        alpha_cutoff: 0.3,
        alpha_mode: AlphaMode::Mask,
        albedo_texture: Some("tex.png".to_string()),
        ..Default::default()
    };
    let bytes = export_rxmat(&original);
    let imported = import_rxmat(&bytes).unwrap();
    assert_eq!(imported.base_color, original.base_color);
    assert_eq!(imported.roughness, original.roughness);
    assert_eq!(imported.metallic, original.metallic);
    assert_eq!(imported.ao, original.ao);
    assert_eq!(imported.emissive, original.emissive);
    assert_eq!(imported.normal_scale, original.normal_scale);
    assert_eq!(imported.occlusion_strength, original.occlusion_strength);
    assert_eq!(imported.alpha_cutoff, original.alpha_cutoff);
    assert_eq!(imported.alpha_mode, original.alpha_mode);
    assert_eq!(imported.albedo_texture, original.albedo_texture);
    assert!(imported.normal_texture.is_none());
}

#[test]
fn rxmat_roundtrip_no_textures() {
    let original = MaterialAsset::default();
    let bytes = export_rxmat(&original);
    let imported = import_rxmat(&bytes).unwrap();
    assert_eq!(imported, original);
}

#[test]
fn rxmat_invalid_magic() {
    let result = import_rxmat(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxmat_too_small() {
    let result = import_rxmat(b"RXA1");
    assert!(result.is_err());
}
