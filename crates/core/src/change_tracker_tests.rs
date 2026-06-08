//! Tests for change tracking.

use std::any::TypeId;
use crate::change_tracker::ChangeTracker;
use crate::ecs::{EcsWorld, Entity};

#[derive(Debug, Clone, PartialEq, Default)]
struct Position {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Velocity {
    dx: f32,
    dy: f32,
}

#[test]
fn tracker_starts_empty() {
    let tracker = ChangeTracker::new();
    assert_eq!(tracker.tracked_type_count(), 0);
    assert_eq!(tracker.total_changed_count(), 0);
}

#[test]
fn tracker_flag_and_check() {
    let mut tracker = ChangeTracker::new();
    let mut world = EcsWorld::new();
    let e = world.spawn(());

    tracker.flag::<Position>(e);
    assert!(tracker.is_changed::<Position>(e));
    assert!(!tracker.is_changed::<Velocity>(e));
}

#[test]
fn tracker_flag_erased() {
    let mut tracker = ChangeTracker::new();
    let mut world = EcsWorld::new();
    let e = world.spawn(());

    tracker.flag_erased(TypeId::of::<Position>(), e);
    assert!(tracker.is_changed_erased(TypeId::of::<Position>(), e));
    assert!(!tracker.is_changed_erased(TypeId::of::<Velocity>(), e));
}

#[test]
fn tracker_changed_entities() {
    let mut tracker = ChangeTracker::new();
    let mut world = EcsWorld::new();
    let e1 = world.spawn(());
    let e2 = world.spawn(());

    tracker.flag::<Position>(e1);
    tracker.flag::<Position>(e2);

    let set = tracker.changed_entities::<Position>().unwrap();
    assert!(set.contains(&e1));
    assert!(set.contains(&e2));
}

#[test]
fn tracker_manual_filter_with_is_changed() {
    let mut tracker = ChangeTracker::new();
    let mut world = EcsWorld::new();
    let e1 = world.spawn((Position { x: 1.0, y: 0.0, z: 0.0 },));
    let e2 = world.spawn((Position { x: 2.0, y: 0.0, z: 0.0 },));
    let e3 = world.spawn((Position { x: 3.0, y: 0.0, z: 0.0 },));

    tracker.flag::<Position>(e1);
    tracker.flag::<Position>(e3);

    let mut changed = Vec::new();
    for (e, _pos) in world.query_mut::<(Entity, &Position)>() {
        if tracker.is_changed::<Position>(e) {
            changed.push(e);
        }
    }
    assert_eq!(changed.len(), 2);
    assert!(changed.contains(&e1));
    assert!(!changed.contains(&e2));
    assert!(changed.contains(&e3));
}

#[test]
fn tracker_clear_removes_all() {
    let mut tracker = ChangeTracker::new();
    let mut world = EcsWorld::new();
    let e = world.spawn(());

    tracker.flag::<Position>(e);
    tracker.flag::<Velocity>(e);
    assert_eq!(tracker.total_changed_count(), 2);

    tracker.clear();
    assert!(!tracker.is_changed::<Position>(e));
    assert!(!tracker.is_changed::<Velocity>(e));
    assert_eq!(tracker.total_changed_count(), 0);
}

#[test]
fn tracker_clear_type_selective() {
    let mut tracker = ChangeTracker::new();
    let mut world = EcsWorld::new();
    let e = world.spawn(());

    tracker.flag::<Position>(e);
    tracker.flag::<Velocity>(e);

    tracker.clear_type::<Position>();
    assert!(!tracker.is_changed::<Position>(e));
    assert!(tracker.is_changed::<Velocity>(e));
}

#[test]
fn tracker_clear_type_erased() {
    let mut tracker = ChangeTracker::new();
    let mut world = EcsWorld::new();
    let e = world.spawn(());

    tracker.flag::<Position>(e);
    tracker.flag::<Velocity>(e);

    tracker.clear_type_erased(TypeId::of::<Position>());
    assert!(!tracker.is_changed::<Position>(e));
    assert!(tracker.is_changed::<Velocity>(e));
}

#[test]
fn tracker_multiple_entities_same_type() {
    let mut tracker = ChangeTracker::new();
    let mut world = EcsWorld::new();
    let e1 = world.spawn(());
    let e2 = world.spawn(());
    let e3 = world.spawn(());

    tracker.flag::<Position>(e1);
    tracker.flag::<Position>(e3);

    assert_eq!(tracker.changed_entities::<Position>().unwrap().len(), 2);
    assert!(tracker.is_changed::<Position>(e1));
    assert!(!tracker.is_changed::<Position>(e2));
    assert!(tracker.is_changed::<Position>(e3));
}

#[test]
fn tracker_duplicate_flag_is_idempotent() {
    let mut tracker = ChangeTracker::new();
    let mut world = EcsWorld::new();
    let e = world.spawn(());

    tracker.flag::<Position>(e);
    tracker.flag::<Position>(e);
    tracker.flag::<Position>(e);

    assert_eq!(tracker.changed_entities::<Position>().unwrap().len(), 1);
}

#[test]
fn tracker_changed_entities_unknown_type_returns_none() {
    let tracker = ChangeTracker::new();
    assert!(tracker.changed_entities::<Position>().is_none());
}
