use super::*;
use rustix_core::ecs::EcsWorld;

#[test]
fn scene_round_trip_preserved_entities() {
    let mut world = EcsWorld::new();
    world.spawn((
        Name("Cube".into()),
        Transform { position: Vec3::new(1.0, 2.0, 3.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Cube".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
    ));
    world.spawn((
        Name("Light".into()),
        Transform { position: Vec3::new(5.0, 10.0, 5.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
        DirectionalLight { color: Vec3::new(1.0, 1.0, 1.0), intensity: 1.0 },
    ));

    let scene = world_to_scene(&world);
    assert_eq!(scene.entities.len(), 2, "world_to_scene should capture 2 entities");

    // Serialize to JSON and back
    let json = serde_json::to_string_pretty(&scene).expect("serialization failed");
    let restored: SceneData = serde_json::from_str(&json).expect("deserialization failed");
    assert_eq!(restored.entities.len(), 2, "deserialized scene should have 2 entities");

    let mut new_world = EcsWorld::new();
    scene_to_world(&mut new_world, &restored);
    let count = new_world.query::<(&Name,)>().iter().count();
    assert_eq!(count, 2, "scene_to_world should spawn 2 entities");

    // Verify names and transforms preserved
    let names: Vec<String> = new_world.query::<&Name>().iter().map(|n| n.0.clone()).collect();
    assert!(names.contains(&"Cube".into()));
    assert!(names.contains(&"Light".into()));
}

#[test]
fn scene_round_trip_audio_source() {
    let mut world = EcsWorld::new();
    let src = rustix_audio::AudioSource {
        position: Vec3::new(1.0, 2.0, 3.0),
        min_distance: 2.0,
        max_distance: 50.0,
        rolloff: 1.5,
    };
    world.spawn((
        Name("Speaker".into()),
        Transform { position: Vec3::new(1.0, 2.0, 3.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Sphere".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
        src,
    ));

    let scene = world_to_scene(&world);
    assert_eq!(scene.entities.len(), 1);
    let e = &scene.entities[0];
    assert!(e.audio_source.is_some());
    let saved = e.audio_source.unwrap();
    assert_eq!(saved.position, src.position);
    assert_eq!(saved.min_distance, src.min_distance);
    assert_eq!(saved.max_distance, src.max_distance);
    assert_eq!(saved.rolloff, src.rolloff);

    let mut new_world = EcsWorld::new();
    scene_to_world(&mut new_world, &scene);
    let mut found = false;
    for s in new_world.query::<&rustix_audio::AudioSource>().iter() {
        found = true;
        assert_eq!(s.position, src.position);
        assert_eq!(s.min_distance, src.min_distance);
        assert_eq!(s.max_distance, src.max_distance);
        assert_eq!(s.rolloff, src.rolloff);
    }
    assert!(found, "AudioSource should be restored");
}

#[test]
fn save_load_scene_file_round_trip() {
    let mut world = EcsWorld::new();
    world.spawn((
        Name("Cube".into()),
        Transform { position: Vec3::new(1.0, 2.0, 3.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Cube".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
    ));
    let scene = world_to_scene(&world);
    let tmp = std::env::temp_dir().join("rustix_test_scene.json");
    assert!(save_scene(&tmp, &scene).is_some());
    let loaded = load_scene(&tmp).expect("load_scene should succeed");
    assert_eq!(loaded.entities.len(), 1);
    assert_eq!(loaded.entities[0].name, "Cube");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn additive_scene_load_preserves_existing_entities() {
    let mut world = EcsWorld::new();
    world.spawn((
        Name("Existing".into()),
        Transform { position: Vec3::new(0.0, 0.0, 0.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Cube".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
    ));

    let scene_data = SceneData {
        entities: vec![
            SceneEntity {
                name: "Added".into(),
                position: [10.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                mesh: Some("Sphere".into()),
                dirlight: None,
                pointlight: None,
                spotlight: None,
                material: None,
                script: None,
                rigidbody: None,
                collider: None,
                audiolistener: None,
                camera: None,
                skeleton: None,
                terrain: None,
                audio_source: None,
                scene_tag: None,
                parent_idx: None,
            },
        ],
    };

    merge_scene_into_world(&mut world, &scene_data, "test_scene");
    let names: Vec<String> = world.query::<&Name>().iter().map(|n| n.0.clone()).collect();
    assert_eq!(names.len(), 2, "additive load should result in 2 entities");
    assert!(names.contains(&"Existing".into()));
    assert!(names.contains(&"Added".into()));
}

#[test]
fn additive_scene_load_tags_entities() {
    let mut world = EcsWorld::new();
    let scene_data = SceneData {
        entities: vec![
            SceneEntity {
                name: "Tagged".into(),
                position: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                mesh: None,
                dirlight: None,
                pointlight: None,
                spotlight: None,
                material: None,
                script: None,
                rigidbody: None,
                collider: None,
                audiolistener: None,
                camera: None,
                skeleton: None,
                terrain: None,
                audio_source: None,
                scene_tag: None,
                parent_idx: None,
            },
        ],
    };

    merge_scene_into_world(&mut world, &scene_data, "my_scene");
    let mut found = false;
    for tag in world.query::<&SceneTag>().iter() {
        assert_eq!(tag.0, "my_scene");
        found = true;
    }
    assert!(found, "SceneTag should be present on loaded entities");
}

#[test]
fn unload_scene_removes_tagged_entities() {
    let mut world = EcsWorld::new();
    world.spawn((
        Name("Keeper".into()),
        Transform { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Cube".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
    ));

    let scene_data = SceneData {
        entities: vec![
            SceneEntity {
                name: "Removable".into(),
                position: [5.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                mesh: None,
                dirlight: None,
                pointlight: None,
                spotlight: None,
                material: None,
                script: None,
                rigidbody: None,
                collider: None,
                audiolistener: None,
                camera: None,
                skeleton: None,
                terrain: None,
                audio_source: None,
                scene_tag: None,
                parent_idx: None,
            },
        ],
    };

    merge_scene_into_world(&mut world, &scene_data, "temp_scene");
    assert_eq!(world.query::<&Name>().iter().count(), 2);

    let removed = unload_scene(&mut world, "temp_scene");
    assert_eq!(removed, 1, "should remove exactly 1 tagged entity");
    assert_eq!(world.query::<&Name>().iter().count(), 1);

    let names: Vec<String> = world.query::<&Name>().iter().map(|n| n.0.clone()).collect();
    assert!(names.contains(&"Keeper".into()));
}

#[test]
fn world_to_scene_by_tag_filters_correctly() {
    let mut world = EcsWorld::new();
    world.spawn((
        Name("Alpha".into()),
        Transform { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Cube".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
        SceneTag("scene_a".into()),
    ));
    world.spawn((
        Name("Beta".into()),
        Transform { position: Vec3::new(1.0, 0.0, 0.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Cube".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
        SceneTag("scene_b".into()),
    ));

    let scene_a = world_to_scene_by_tag(&world, "scene_a");
    assert_eq!(scene_a.entities.len(), 1);
    assert_eq!(scene_a.entities[0].name, "Alpha");

    let scene_b = world_to_scene_by_tag(&world, "scene_b");
    assert_eq!(scene_b.entities.len(), 1);
    assert_eq!(scene_b.entities[0].name, "Beta");
}

#[test]
fn streaming_zone_evaluation() {
    let mut zones = vec![
        StreamingZone {
            name: "Zone1".into(),
            center: Vec3::new(0.0, 0.0, 0.0),
            radius: 10.0,
            scene_path: "zone1.rustixscene".into(),
            loaded: false,
        },
    ];

    // Viewer inside zone -> should trigger load
    let changes = evaluate_streaming(&mut zones, Vec3::new(5.0, 0.0, 0.0));
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].0, "zone1.rustixscene");
    assert!(changes[0].1);
    assert!(zones[0].loaded);

    // Viewer still inside -> no new changes
    let changes = evaluate_streaming(&mut zones, Vec3::new(3.0, 0.0, 0.0));
    assert_eq!(changes.len(), 0);

    // Viewer outside -> should trigger unload
    let changes = evaluate_streaming(&mut zones, Vec3::new(20.0, 0.0, 0.0));
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].0, "zone1.rustixscene");
    assert!(!changes[0].1);
    assert!(!zones[0].loaded);
}
