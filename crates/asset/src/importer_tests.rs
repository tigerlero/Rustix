//! Tests for importer registries and serialization helpers.

use crate::importer::{ImporterRegistry, ReloadRegistry, import_ron, import_json, export_ron, export_json};
use crate::material::MaterialAsset;

#[test]
fn importer_registry_new_empty() {
    let reg = ImporterRegistry::new();
    assert!(reg.find_for_extension("png").is_none());
}

#[test]
fn importer_registry_register_and_find() {
    let mut reg = ImporterRegistry::new();
    reg.register::<MaterialAsset>("mat", std::any::TypeId::of::<MaterialAsset>(), "material");
    let found = reg.find_for_extension("mat");
    assert!(found.is_some());
    let (_, name) = found.unwrap();
    assert_eq!(name, "material");
}

#[test]
fn importer_registry_unknown_extension() {
    let mut reg = ImporterRegistry::new();
    reg.register::<MaterialAsset>("mat", std::any::TypeId::of::<MaterialAsset>(), "material");
    assert!(reg.find_for_extension("png").is_none());
}

#[test]
fn reload_registry_new_empty() {
    let reg = ReloadRegistry::new();
    assert!(reg.find_for_extension("png").is_none());
    assert!(reg.reload("png", b"", None).is_none());
}

#[test]
fn export_import_ron_roundtrip() {
    let original = MaterialAsset::default();
    let ron_str = export_ron(&original).unwrap();
    let imported: MaterialAsset = import_ron(ron_str.as_bytes()).unwrap();
    assert_eq!(imported.base_color, original.base_color);
    assert_eq!(imported.roughness, original.roughness);
}

#[test]
fn export_import_json_roundtrip() {
    let original = MaterialAsset::default();
    let json_str = export_json(&original).unwrap();
    let imported: MaterialAsset = import_json(json_str.as_bytes()).unwrap();
    assert_eq!(imported.base_color, original.base_color);
    assert_eq!(imported.roughness, original.roughness);
}

#[test]
fn import_ron_invalid_utf8() {
    let bytes = vec![0xFF, 0xFE];
    let result: Result<MaterialAsset, _> = import_ron(&bytes);
    assert!(result.is_err());
}

#[test]
fn import_json_invalid_utf8() {
    let bytes = vec![0xFF, 0xFE];
    let result: Result<MaterialAsset, _> = import_json(&bytes);
    assert!(result.is_err());
}

#[test]
fn import_ron_bad_syntax() {
    let result: Result<MaterialAsset, _> = import_ron(b"not valid ron");
    assert!(result.is_err());
}

#[test]
fn import_json_bad_syntax() {
    let result: Result<MaterialAsset, _> = import_json(b"not valid json");
    assert!(result.is_err());
}
