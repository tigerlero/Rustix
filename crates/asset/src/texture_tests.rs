//! Tests for texture asset types and binary format.

use crate::texture::{TextureAsset, TextureFormat, import_rxtex, export_rxtex};

#[test]
fn texture_format_from_u32() {
    assert_eq!(TextureFormat::from_u32(0), Some(TextureFormat::R8g8b8a8Unorm));
    assert_eq!(TextureFormat::from_u32(1), Some(TextureFormat::R16g16b16a16Sfloat));
    assert_eq!(TextureFormat::from_u32(2), Some(TextureFormat::R32g32b32a32Sfloat));
    assert_eq!(TextureFormat::from_u32(99), None);
}

#[test]
fn texture_format_bytes_per_pixel() {
    assert_eq!(TextureFormat::R8g8b8a8Unorm.bytes_per_pixel(), 4);
    assert_eq!(TextureFormat::R16g16b16a16Sfloat.bytes_per_pixel(), 8);
    assert_eq!(TextureFormat::R32g32b32a32Sfloat.bytes_per_pixel(), 16);
}

#[test]
fn texture_asset_new() {
    let pixels = vec![0u8; 16]; // 2x2 RGBA8
    let tex = TextureAsset::new(2, 2, TextureFormat::R8g8b8a8Unorm, pixels);
    assert_eq!(tex.width, 2);
    assert_eq!(tex.height, 2);
    assert_eq!(tex.format, TextureFormat::R8g8b8a8Unorm);
    assert_eq!(tex.mip_levels, 1);
}

#[test]
#[should_panic(expected = "pixel buffer size mismatch")]
fn texture_asset_new_wrong_size() {
    let pixels = vec![0u8; 8]; // too small for 2x2 RGBA8
    let _ = TextureAsset::new(2, 2, TextureFormat::R8g8b8a8Unorm, pixels);
}

#[test]
fn texture_asset_with_mips() {
    let pixels = vec![0u8; 16];
    let tex = TextureAsset::new(2, 2, TextureFormat::R8g8b8a8Unorm, pixels).with_mips(4);
    assert_eq!(tex.mip_levels, 4);
}

#[test]
fn rxtex_roundtrip() {
    let pixels = vec![0xFFu8, 0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let original = TextureAsset::new(2, 2, TextureFormat::R8g8b8a8Unorm, pixels.clone());
    let bytes = export_rxtex(&original);
    let imported = import_rxtex(&bytes).unwrap();
    assert_eq!(imported.width, original.width);
    assert_eq!(imported.height, original.height);
    assert_eq!(imported.format, original.format);
    assert_eq!(imported.mip_levels, original.mip_levels);
    assert_eq!(imported.pixels, pixels);
}

#[test]
fn rxtex_invalid_magic() {
    let result = import_rxtex(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxtex_too_small() {
    let result = import_rxtex(b"RXT1");
    assert!(result.is_err());
}

#[test]
fn rxtex_unknown_format() {
    let mut bytes = b"RXT1".to_vec();
    bytes.extend_from_slice(&1u32.to_le_bytes()); // version
    bytes.extend_from_slice(&1u32.to_le_bytes()); // width
    bytes.extend_from_slice(&1u32.to_le_bytes()); // height
    bytes.extend_from_slice(&99u32.to_le_bytes()); // invalid format
    bytes.extend_from_slice(&1u32.to_le_bytes()); // mip_levels
    let result = import_rxtex(&bytes);
    assert!(result.is_err());
}
