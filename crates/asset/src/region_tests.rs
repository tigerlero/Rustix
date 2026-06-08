//! Tests for region asset types and binary format.

use crate::region::*;
use crate::prefab::{PrefabEntity, PrefabVec3};

#[test]
fn region_metadata_default() {
    let meta = RegionMetadata::default();
    assert_eq!(meta.name, "Untitled Region");
    assert_eq!(meta.ambient_color, [0.1, 0.1, 0.1]);
    assert_eq!(meta.ambient_intensity, 0.3);
    assert_eq!(meta.sky_color, [0.5, 0.7, 1.0]);
    assert_eq!(meta.fog_density, 0.0);
}

#[test]
fn region_asset_new_and_count() {
    let data = RegionData {
        metadata: RegionMetadata::default(),
        entities: vec![
            PrefabEntity { name: "player".to_string(), position: [0.0; 3], rotation: [0.0; 3], scale: [1.0; 3], mesh: None, material: None, dirlight: None, pointlight: None, spotlight: None, script: None, rigidbody: None, collider: None, audiolistener: None, audiosource: None, camera: None, parent_idx: None },
        ],
    };
    let region = RegionAsset::new(data);
    assert_eq!(region.entity_count(), 1);
}

#[test]
fn rxregion_roundtrip() {
    let data = RegionData {
        metadata: RegionMetadata {
            name: "TestLevel".to_string(),
            ambient_color: [0.2, 0.2, 0.3],
            ambient_intensity: 0.5,
            sky_color: [0.3, 0.5, 0.8],
            fog_color: [0.5, 0.5, 0.5],
            fog_density: 0.01,
        },
        entities: vec![
            PrefabEntity {
                name: "cube".to_string(),
                position: [10.0, 0.0, 5.0],
                rotation: [0.0; 3],
                scale: [1.0; 3],
                mesh: Some("cube.rxmesh".to_string()),
                material: None,
                dirlight: None,
                pointlight: None,
                spotlight: None,
                script: None,
                rigidbody: None,
                collider: None,
                audiolistener: None,
                audiosource: None,
                camera: None,
                parent_idx: None,
            },
        ],
    };
    let original = RegionAsset::new(data);
    let bytes = export_rxregion(&original);
    let imported = import_rxregion(&bytes).unwrap();
    assert_eq!(imported.entity_count(), 1);
    assert_eq!(imported.data.metadata.name, "TestLevel");
    assert_eq!(imported.data.metadata.ambient_intensity, 0.5);
    assert_eq!(imported.data.entities[0].name, "cube");
    assert_eq!(imported.data.entities[0].position, [10.0, 0.0, 5.0]);
}

#[test]
fn rxregion_empty_roundtrip() {
    let original = RegionAsset::new(RegionData {
        metadata: RegionMetadata::default(),
        entities: vec![],
    });
    let bytes = export_rxregion(&original);
    let imported = import_rxregion(&bytes).unwrap();
    assert_eq!(imported.entity_count(), 0);
    assert_eq!(imported.data.metadata.name, "Untitled Region");
}

#[test]
fn rxregion_invalid_magic() {
    let result = import_rxregion(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxregion_too_small() {
    let result = import_rxregion(b"RXR1");
    assert!(result.is_err());
}
