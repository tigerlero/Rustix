//! Tests for multi-scene manager.

use crate::multi_scene::{Scene, SceneManager};

#[test]
fn scene_new_defaults() {
    let scene = Scene::new("test");
    assert_eq!(scene.name, "test");
    assert!(scene.active);
    assert!(scene.loaded);
}

#[test]
fn scene_manager_new_has_main() {
    let mgr = SceneManager::new("main");
    assert_eq!(mgr.scenes.len(), 1);
    assert_eq!(mgr.main_scene, 0);
    assert_eq!(mgr.main().name, "main");
}

#[test]
fn scene_manager_load_scene() {
    let mut mgr = SceneManager::new("main");
    let idx = mgr.load_scene("ui");
    assert_eq!(idx, 1);
    assert_eq!(mgr.scenes.len(), 2);
    assert_eq!(mgr.scenes[idx].name, "ui");
}

#[test]
fn scene_manager_unload_scene() {
    let mut mgr = SceneManager::new("main");
    let idx = mgr.load_scene("sub");
    mgr.unload_scene(idx);
    assert!(!mgr.scenes[idx].loaded);
    assert!(!mgr.scenes[idx].active);
}

#[test]
fn scene_manager_set_active() {
    let mut mgr = SceneManager::new("main");
    mgr.set_active(0, false);
    assert!(!mgr.scenes[0].active);
}

#[test]
fn scene_manager_active_scenes() {
    let mut mgr = SceneManager::new("main");
    let idx = mgr.load_scene("sub");
    mgr.unload_scene(idx);
    let active = mgr.active_scenes();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "main");
}

#[test]
fn scene_manager_active_scenes_mut() {
    let mut mgr = SceneManager::new("main");
    let mut active = mgr.active_scenes_mut();
    assert_eq!(active.len(), 1);
    active[0].name = "modified".to_string();
    assert_eq!(mgr.scenes[0].name, "modified");
}
