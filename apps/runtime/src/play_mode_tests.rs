use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;

use crate::play_mode::PlayModeSnapshot;
use crate::scene::{world_to_scene, scene_to_world, SceneTag};
use crate::player::PlayerManager;
use crate::scene::SceneManager;
use crate::camera::{EditorCamera, CameraMode};

fn make_test_world() -> EcsWorld {
    let mut world = EcsWorld::new();
    world.spawn((
        crate::scene::Name("Camera".into()),
        crate::scene::Transform { position: Vec3::new(0.0, 5.0, 10.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
        crate::scene::MeshComponent("Cube".into()),
        crate::scene::Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
    ));
    world.spawn((
        crate::scene::Name("Light".into()),
        crate::scene::Transform { position: Vec3::new(5.0, 10.0, 5.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
        rustix_render::DirectionalLight { color: Vec3::new(1.0, 1.0, 1.0), intensity: 1.0 },
    ));
    world
}

fn make_test_camera() -> EditorCamera {
    EditorCamera {
        position: Vec3::new(1.0, 2.0, 3.0),
        center: Vec3::new(0.0, 0.0, 0.0),
        yaw: 0.5,
        pitch: -0.3,
        distance: 10.0,
        mode: CameraMode::Orbit,
        follow_target: false,
        controlling_player: false,
    }
}

#[test]
fn snapshot_captures_entity_count() {
    let world = make_test_world();
    let camera = make_test_camera();
    let player_mgr = PlayerManager::new();
    let scene_mgr = SceneManager::new();

    let snapshot = PlayModeSnapshot::capture(&world, &camera, &player_mgr, &scene_mgr);
    assert_eq!(snapshot.scene_data.entities.len(), 2, "snapshot should capture 2 entities");
}

#[test]
fn snapshot_restores_entities() {
    let mut world = make_test_world();
    let camera = make_test_camera();
    let player_mgr = PlayerManager::new();
    let scene_mgr = SceneManager::new();

    let snapshot = PlayModeSnapshot::capture(&world, &camera, &player_mgr, &scene_mgr);

    // Mutate world in "play-mode"
    world.clear();
    world.spawn((
        crate::scene::Name("PlayEntity".into()),
        crate::scene::Transform { position: Vec3::ONE, rotation: Vec3::ZERO, scale: Vec3::ONE },
        crate::scene::MeshComponent("Sphere".into()),
        crate::scene::Material { base_color: Vec3::new(1.0, 0.0, 0.0), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
    ));

    let mut restored_camera = EditorCamera {
        position: Vec3::ZERO,
        center: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        distance: 5.0,
        mode: CameraMode::FirstPerson,
        follow_target: true,
        controlling_player: true,
    };
    let mut restored_player = PlayerManager::new();
    let mut restored_scene = SceneManager::new();

    snapshot.restore(&mut world, &mut restored_camera, &mut restored_player, &mut restored_scene);

    let names: Vec<String> = world.query::<&crate::scene::Name>().iter().map(|n| n.0.clone()).collect();
    assert_eq!(names.len(), 2, "restore should bring back 2 original entities");
    assert!(names.contains(&"Camera".into()));
    assert!(names.contains(&"Light".into()));
}

#[test]
fn snapshot_restores_camera_state() {
    let world = make_test_world();
    let camera = make_test_camera();
    let player_mgr = PlayerManager::new();
    let scene_mgr = SceneManager::new();

    let snapshot = PlayModeSnapshot::capture(&world, &camera, &player_mgr, &scene_mgr);

    let mut restored_camera = EditorCamera {
        position: Vec3::ZERO,
        center: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        distance: 0.0,
        mode: CameraMode::FirstPerson,
        follow_target: true,
        controlling_player: true,
    };
    let mut world2 = make_test_world();
    let mut restored_player = PlayerManager::new();
    let mut restored_scene = SceneManager::new();

    snapshot.restore(&mut world2, &mut restored_camera, &mut restored_player, &mut restored_scene);

    assert_eq!(restored_camera.position, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(restored_camera.yaw, 0.5);
    assert_eq!(restored_camera.pitch, -0.3);
    assert_eq!(restored_camera.distance, 10.0);
    assert_eq!(restored_camera.mode, CameraMode::Orbit);
    assert!(!restored_camera.follow_target);
    assert!(!restored_camera.controlling_player);
}

#[test]
fn snapshot_restores_scene_tags() {
    let mut world = EcsWorld::new();
    world.spawn((
        crate::scene::Name("Tagged".into()),
        crate::scene::Transform { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE },
        crate::scene::MeshComponent("Cube".into()),
        crate::scene::Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
        SceneTag("my_level".into()),
    ));

    let camera = make_test_camera();
    let player_mgr = PlayerManager::new();
    let scene_mgr = SceneManager::new();

    let snapshot = PlayModeSnapshot::capture(&world, &camera, &player_mgr, &scene_mgr);
    world.clear();

    let mut restored_camera = make_test_camera();
    let mut restored_player = PlayerManager::new();
    let mut restored_scene = SceneManager::new();

    snapshot.restore(&mut world, &mut restored_camera, &mut restored_player, &mut restored_scene);

    let mut found = false;
    for tag in world.query::<&SceneTag>().iter() {
        assert_eq!(tag.0, "my_level");
        found = true;
    }
    assert!(found, "SceneTag should survive snapshot round-trip");
}

#[test]
fn snapshot_restores_scene_manager_state() {
    let world = make_test_world();
    let camera = make_test_camera();
    let player_mgr = PlayerManager::new();
    let mut scene_mgr = SceneManager::new();
    scene_mgr.register("level_a".into(), "level_a.rustixscene".into(), 12);

    let snapshot = PlayModeSnapshot::capture(&world, &camera, &player_mgr, &scene_mgr);

    let mut world2 = make_test_world();
    let mut restored_camera = make_test_camera();
    let mut restored_player = PlayerManager::new();
    let mut restored_scene = SceneManager::new();

    snapshot.restore(&mut world2, &mut restored_camera, &mut restored_player, &mut restored_scene);

    assert!(restored_scene.is_loaded("level_a"));
    assert_eq!(restored_scene.loaded_scenes[0].entity_count, 12);
}
