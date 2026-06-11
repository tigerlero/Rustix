use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;

use crate::scene::*;
use crate::scene::{SceneEntity, SceneData, SceneTag, SceneManager, merge_scene_into_world, unload_scene, world_to_scene_by_tag, evaluate_streaming, StreamingZone};

fn make_test_scene_entity(name: &str, pos: [f32; 3]) -> SceneEntity {
    SceneEntity {
        name: name.into(),
        position: pos,
        rotation: [0.0, 0.0, 0.0],
        scale: [1.0, 1.0, 1.0],
        mesh: Some("Cube".into()),
        dirlight: None,
        pointlight: None,
        spotlight: None,
        material: Some(Material {
            base_color: Vec3::new(0.7, 0.7, 0.7),
            alpha: 1.0,
            roughness: 0.5,
            metallic: 0.0,
            ao: 1.0,
            emissive: 0.0,
        }),
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
    }
}

#[test]
fn additive_scene_load_preserves_existing_entities() {
    let mut world = EcsWorld::new();
    world.spawn((
        Name("Existing".into()),
        Transform { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Cube".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
    ));

    let scene_data = SceneData {
        entities: vec![make_test_scene_entity("Added", [10.0, 0.0, 0.0])],
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
        entities: vec![make_test_scene_entity("Tagged", [1.0, 2.0, 3.0])],
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
        entities: vec![make_test_scene_entity("Removable", [5.0, 0.0, 0.0])],
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
fn unload_scene_also_removes_children_of_tagged_parents() {
    let mut world = EcsWorld::new();
    let parent = world.spawn((
        Name("Parent".into()),
        Transform { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Cube".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
        SceneTag("scene_a".into()),
    ));
    let _child = world.spawn((
        Name("Child".into()),
        Transform { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Cube".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
        Parent(Some(parent)),
    ));

    assert_eq!(world.query::<&Name>().iter().count(), 2);
    let removed = unload_scene(&mut world, "scene_a");
    assert_eq!(removed, 2, "should remove parent and child");
    assert_eq!(world.query::<&Name>().iter().count(), 0);
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
fn streaming_zone_evaluation_load_and_unload() {
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

#[test]
fn scene_manager_tracks_loaded_scenes() {
    let mut mgr = SceneManager::new();
    assert!(!mgr.is_loaded("level1"));
    mgr.register("level1".into(), "/path/level1.rustixscene".into(), 42);
    assert!(mgr.is_loaded("level1"));
    assert_eq!(mgr.loaded_scenes[0].entity_count, 42);
    mgr.register("level1".into(), "/path/level1.rustixscene".into(), 50);
    assert_eq!(mgr.loaded_scenes[0].entity_count, 50);
    mgr.unregister("level1");
    assert!(!mgr.is_loaded("level1"));
}

#[test]
fn scene_manager_multiple_scenes() {
    let mut mgr = SceneManager::new();
    mgr.register("forest".into(), "forest.rustixscene".into(), 10);
    mgr.register("cave".into(), "cave.rustixscene".into(), 5);
    assert!(mgr.is_loaded("forest"));
    assert!(mgr.is_loaded("cave"));
    mgr.unregister("forest");
    assert!(!mgr.is_loaded("forest"));
    assert!(mgr.is_loaded("cave"));
}
