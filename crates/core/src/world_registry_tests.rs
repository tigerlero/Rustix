//! Tests for world registry.

use crate::world_registry::{WorldRegistry, EntityMapping};
use crate::ecs::EcsWorld as HecsWorld;

#[test]
fn registry_starts_empty() {
    let reg = WorldRegistry::new();
    assert!(reg.is_empty());
    assert_eq!(reg.len(), 0);
    assert!(reg.active().is_none());
}

#[test]
fn registry_create_makes_active() {
    let mut reg = WorldRegistry::new();
    let world = reg.create("game");
    let e = world.spawn((42i32,));
    assert_eq!(reg.active().unwrap().query::<&i32>().iter().count(), 1);
    assert!(reg.has("game"));
    assert!(!reg.has("editor"));
}

#[test]
fn registry_create_inactive() {
    let mut reg = WorldRegistry::new();
    reg.create("game");
    reg.create_inactive("editor");
    assert_eq!(reg.len(), 2);
    assert_eq!(reg.active().unwrap().query::<&i32>().iter().count(), 0);
}

#[test]
fn registry_set_active() {
    let mut reg = WorldRegistry::new();
    reg.create("game");
    reg.create_inactive("editor");
    reg.get_mut("editor").unwrap().spawn((42i32,));
    reg.set_active("editor");
    assert_eq!(reg.active().unwrap().query::<&i32>().iter().count(), 1);
}

#[test]
#[should_panic(expected = "world 'ghost' does not exist")]
fn registry_set_active_unknown_panics() {
    let mut reg = WorldRegistry::new();
    reg.set_active("ghost");
}

#[test]
fn registry_destroy_removes_world() {
    let mut reg = WorldRegistry::new();
    reg.create("game");
    assert!(reg.destroy("game"));
    assert!(!reg.has("game"));
    assert!(reg.active().is_none());
}

#[test]
fn registry_destroy_unknown_returns_false() {
    let mut reg = WorldRegistry::new();
    assert!(!reg.destroy("ghost"));
}

#[test]
fn registry_get_and_get_mut() {
    let mut reg = WorldRegistry::new();
    reg.create("game");
    assert!(reg.get("game").is_some());
    reg.get_mut("game").unwrap().spawn((42i32,));
    assert_eq!(reg.get("game").unwrap().query::<&i32>().iter().count(), 1);
}

#[test]
fn registry_spawn_active() {
    let mut reg = WorldRegistry::new();
    reg.create("game");
    let e = reg.spawn_active((42i32,)).unwrap();
    assert!(reg.active().unwrap().satisfies::<&i32>(e));
}

#[test]
fn registry_spawn_active_none_when_no_active() {
    let mut reg = WorldRegistry::new();
    reg.create_inactive("game");
    assert!(reg.spawn_active((42i32,)).is_none());
}

#[test]
fn registry_names() {
    let mut reg = WorldRegistry::new();
    reg.create("game");
    reg.create_inactive("editor");
    let mut names: Vec<_> = reg.names().collect();
    names.sort();
    assert_eq!(names, vec!["editor", "game"]);
}

#[test]
fn registry_clear() {
    let mut reg = WorldRegistry::new();
    reg.create("game");
    reg.create_inactive("editor");
    reg.clear();
    assert!(reg.is_empty());
    assert!(reg.active().is_none());
}

// ------------------------------------------------------------------
// EntityMapping tests
// ------------------------------------------------------------------

#[test]
fn mapping_insert_and_get() {
    let mut map = EntityMapping::new();
    let mut w = HecsWorld::new();
    let e1 = w.spawn(());
    let e2 = w.spawn(());
    map.insert(e1, e2);
    assert_eq!(map.get(e1), Some(e2));
    assert_eq!(map.get_reverse(e2), Some(e1));
}

#[test]
fn mapping_overwrite_existing() {
    let mut map = EntityMapping::new();
    let mut w = HecsWorld::new();
    let e1 = w.spawn(());
    let e2 = w.spawn(());
    let e3 = w.spawn(());
    map.insert(e1, e2);
    map.insert(e1, e3);
    assert_eq!(map.get(e1), Some(e3));
    assert!(map.get(e2).is_none());
    assert_eq!(map.get_reverse(e3), Some(e1));
}

#[test]
fn mapping_remove() {
    let mut map = EntityMapping::new();
    let mut w = HecsWorld::new();
    let e1 = w.spawn(());
    let e2 = w.spawn(());
    map.insert(e1, e2);
    assert_eq!(map.remove(e1), Some(e2));
    assert!(map.get(e1).is_none());
    assert!(map.get_reverse(e2).is_none());
}

#[test]
fn mapping_len_and_clear() {
    let mut map = EntityMapping::new();
    let mut w = HecsWorld::new();
    let e1 = w.spawn(());
    let e2 = w.spawn(());
    map.insert(e1, e2);
    assert_eq!(map.len(), 1);
    map.clear();
    assert!(map.is_empty());
}
