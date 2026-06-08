//! Tests for prefab asset types and binary format.

use crate::prefab::*;

#[test]
fn prefab_vec3_from_array() {
    let v: PrefabVec3 = [1.0, 2.0, 3.0].into();
    assert_eq!(v.x, 1.0);
    assert_eq!(v.y, 2.0);
    assert_eq!(v.z, 3.0);
}

#[test]
fn prefab_vec3_to_array() {
    let v = PrefabVec3 { x: 1.0, y: 2.0, z: 3.0 };
    let arr: [f32; 3] = v.into();
    assert_eq!(arr, [1.0, 2.0, 3.0]);
}

#[test]
fn prefab_body_type_default() {
    assert_eq!(PrefabBodyType::default(), PrefabBodyType::Dynamic);
}

#[test]
fn prefab_entity_default_fields() {
    let entity = PrefabEntity {
        name: "test".to_string(),
        position: [0.0; 3],
        rotation: [0.0; 3],
        scale: [1.0; 3],
        mesh: None,
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
    };
    assert_eq!(entity.name, "test");
    assert!(entity.mesh.is_none());
    assert!(entity.parent_idx.is_none());
}

#[test]
fn prefab_asset_new_and_count() {
    let data = PrefabData { entities: vec![
        PrefabEntity { name: "a".to_string(), position: [0.0; 3], rotation: [0.0; 3], scale: [1.0; 3], mesh: None, material: None, dirlight: None, pointlight: None, spotlight: None, script: None, rigidbody: None, collider: None, audiolistener: None, audiosource: None, camera: None, parent_idx: None },
        PrefabEntity { name: "b".to_string(), position: [0.0; 3], rotation: [0.0; 3], scale: [1.0; 3], mesh: None, material: None, dirlight: None, pointlight: None, spotlight: None, script: None, rigidbody: None, collider: None, audiolistener: None, audiosource: None, camera: None, parent_idx: Some(0) },
    ]};
    let prefab = PrefabAsset::new(data);
    assert_eq!(prefab.entity_count(), 2);
}

#[test]
fn rxprefab_roundtrip() {
    let data = PrefabData { entities: vec![
        PrefabEntity {
            name: "root".to_string(),
            position: [1.0, 2.0, 3.0],
            rotation: [0.0; 3],
            scale: [1.0; 3],
            mesh: Some("mesh.rxmesh".to_string()),
            material: Some(PrefabMaterial { base_color: PrefabVec3 { x: 1.0, y: 0.0, z: 0.0 }, alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 }),
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
    ]};
    let original = PrefabAsset::new(data);
    let bytes = export_rxprefab(&original);
    let imported = import_rxprefab(&bytes).unwrap();
    assert_eq!(imported.entity_count(), 1);
    assert_eq!(imported.data.entities[0].name, "root");
    assert_eq!(imported.data.entities[0].position, [1.0, 2.0, 3.0]);
    assert_eq!(imported.data.entities[0].mesh, Some("mesh.rxmesh".to_string()));
}

#[test]
fn rxprefab_empty_roundtrip() {
    let original = PrefabAsset::new(PrefabData { entities: vec![] });
    let bytes = export_rxprefab(&original);
    let imported = import_rxprefab(&bytes).unwrap();
    assert_eq!(imported.entity_count(), 0);
}

#[test]
fn rxprefab_invalid_magic() {
    let result = import_rxprefab(b"BAD1");
    assert!(result.is_err());
}

#[test]
fn rxprefab_too_small() {
    let result = import_rxprefab(b"RXP1");
    assert!(result.is_err());
}
