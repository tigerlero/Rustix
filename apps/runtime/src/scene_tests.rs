use super::*;
use rustix_core::ecs::EcsWorld;

#[test]
fn scene_round_trip_preserved_entities() {
    let mut world = EcsWorld::new();
    world.spawn((
        Name("Cube".into()),
        Transform { position: Vec3::new(1.0, 2.0, 3.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
        MeshComponent("Cube".into()),
        Material { base_color: Vec3::new(0.7, 0.7, 0.7), roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
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
