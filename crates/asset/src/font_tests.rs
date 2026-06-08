//! Tests for font asset types and binary format.

use crate::font::{FontAsset, import_rxfont, export_rxfont};

#[test]
fn font_asset_new() {
    let asset = FontAsset::new("Arial", vec![0xAB, 0xCD]);
    assert_eq!(asset.name, "Arial");
    assert_eq!(asset.data, vec![0xAB, 0xCD]);
}

#[test]
fn rxfont_roundtrip() {
    let original = FontAsset::new("TestFont", vec![0x00, 0x01, 0x02, 0x03, 0xFF]);
    let bytes = export_rxfont(&original);
    let imported = import_rxfont(&bytes).unwrap();
    assert_eq!(imported.name, original.name);
    assert_eq!(imported.data, original.data);
}

#[test]
fn rxfont_empty_data_roundtrip() {
    let original = FontAsset::new("Empty", vec![]);
    let bytes = export_rxfont(&original);
    let imported = import_rxfont(&bytes).unwrap();
    assert_eq!(imported.name, "Empty");
    assert!(imported.data.is_empty());
}

#[test]
fn rxfont_invalid_magic() {
    let result = import_rxfont(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxfont_too_small() {
    let result = import_rxfont(b"RXF1");
    assert!(result.is_err());
}

#[test]
fn rxfont_unsupported_version() {
    let mut bytes = b"RXF1".to_vec();
    bytes.extend_from_slice(&99u32.to_le_bytes());
    let result = import_rxfont(&bytes);
    assert!(result.is_err());
}
