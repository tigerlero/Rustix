//! Tests for physics asset types and binary format.

use crate::physics::{PhysicsMaterialAsset, import_rxphys, export_rxphys};

#[test]
fn physics_material_default() {
    let mat = PhysicsMaterialAsset::default();
    assert_eq!(mat.static_friction, 0.5);
    assert_eq!(mat.dynamic_friction, 0.5);
    assert_eq!(mat.restitution, 0.5);
    assert_eq!(mat.density, 1.0);
}

#[test]
fn physics_material_clone_copy() {
    let mat = PhysicsMaterialAsset::default();
    let cloned = mat.clone();
    assert_eq!(mat, cloned);
}

#[test]
fn rxphys_roundtrip() {
    let original = PhysicsMaterialAsset {
        static_friction: 0.8,
        dynamic_friction: 0.6,
        restitution: 0.3,
        density: 2.5,
    };
    let bytes = export_rxphys(&original);
    let imported = import_rxphys(&bytes).unwrap();
    assert_eq!(imported.static_friction, original.static_friction);
    assert_eq!(imported.dynamic_friction, original.dynamic_friction);
    assert_eq!(imported.restitution, original.restitution);
    assert_eq!(imported.density, original.density);
}

#[test]
fn rxphys_default_roundtrip() {
    let original = PhysicsMaterialAsset::default();
    let bytes = export_rxphys(&original);
    let imported = import_rxphys(&bytes).unwrap();
    assert_eq!(imported, original);
}

#[test]
fn rxphys_invalid_magic() {
    let result = import_rxphys(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxphys_too_small() {
    let result = import_rxphys(b"RXP1");
    assert!(result.is_err());
}

#[test]
fn rxphys_unsupported_version() {
    let mut bytes = b"RXP1".to_vec();
    bytes.extend_from_slice(&99u32.to_le_bytes());
    let result = import_rxphys(&bytes);
    assert!(result.is_err());
}
