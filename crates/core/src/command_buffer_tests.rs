//! Tests for ECS command buffers.

use std::any::TypeId;
use crate::command_buffer::CommandBuffer;
use crate::component_registry::{ComponentRegistry, DynamicBundle};
use crate::ecs::EcsWorld;

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

fn setup() -> (EcsWorld, ComponentRegistry) {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    reg.register::<Velocity>();
    (EcsWorld::new(), reg)
}

#[test]
fn buffer_starts_empty() {
    let buf = CommandBuffer::new();
    assert!(buf.is_empty());
    assert_eq!(buf.len(), 0);
}

#[test]
fn buffer_apply_clears() {
    let mut buf = CommandBuffer::new();
    buf.spawn_empty();
    assert!(!buf.is_empty());

    let (mut world, reg) = setup();
    buf.apply(&mut world, &reg).unwrap();
    assert!(buf.is_empty());
}

#[test]
fn buffer_spawn_empty() {
    let (mut world, reg) = setup();
    let mut buf = CommandBuffer::new();
    buf.spawn_empty();
    buf.apply(&mut world, &reg).unwrap();
    assert_eq!(world.query_mut::<()>().into_iter().count(), 1);
}

#[test]
fn buffer_spawn_with_bundle() {
    let (mut world, reg) = setup();
    let mut bundle = DynamicBundle::new();
    bundle.add(Position { x: 1.0, y: 2.0, z: 3.0 });

    let mut buf = CommandBuffer::new();
    buf.spawn(bundle);
    buf.apply(&mut world, &reg).unwrap();

    assert_eq!(world.query_mut::<&Position>().into_iter().count(), 1);
}

#[test]
fn buffer_despawn_entity() {
    let (mut world, reg) = setup();
    let entity = world.spawn((Position::default(),));

    let mut buf = CommandBuffer::new();
    buf.despawn(entity);
    buf.apply(&mut world, &reg).unwrap();

    assert_eq!(world.query_mut::<&Position>().into_iter().count(), 0);
}

#[test]
fn buffer_insert_bundle() {
    let (mut world, reg) = setup();
    let entity = world.spawn(());

    let mut bundle = DynamicBundle::new();
    bundle.add(Position { x: 1.0, y: 0.0, z: 0.0 });
    bundle.add(Velocity { dx: 2.0, dy: 3.0 });

    let mut buf = CommandBuffer::new();
    buf.insert_bundle(entity, bundle);
    buf.apply(&mut world, &reg).unwrap();

    assert!(world.satisfies::<&Position>(entity));
    assert!(world.satisfies::<&Velocity>(entity));
}

#[test]
fn buffer_insert_one() {
    let (mut world, reg) = setup();
    let entity = world.spawn(());

    let mut buf = CommandBuffer::new();
    buf.insert_one(entity, Position { x: 5.0, y: 0.0, z: 0.0 });
    buf.apply(&mut world, &reg).unwrap();

    assert!(world.satisfies::<&Position>(entity));
}

#[test]
fn buffer_remove_by_type_id() {
    let (mut world, reg) = setup();
    let entity = world.spawn((Position::default(), Velocity::default()));

    let mut buf = CommandBuffer::new();
    buf.remove_by_type_id(entity, TypeId::of::<Position>());
    buf.apply(&mut world, &reg).unwrap();

    assert!(!world.satisfies::<&Position>(entity));
    assert!(world.satisfies::<&Velocity>(entity));
}

#[test]
fn buffer_remove_by_name() {
    let (mut world, reg) = setup();
    let entity = world.spawn((Position::default(), Velocity::default()));

    let mut buf = CommandBuffer::new();
    buf.remove_by_name(entity, "Velocity");
    buf.apply(&mut world, &reg).unwrap();

    assert!(world.satisfies::<&Position>(entity));
    assert!(!world.satisfies::<&Velocity>(entity));
}

#[test]
fn buffer_add_default_by_name() {
    let (mut world, reg) = setup();
    let entity = world.spawn(());

    let mut buf = CommandBuffer::new();
    buf.add_default_by_name(entity, "Position");
    buf.apply(&mut world, &reg).unwrap();

    assert!(world.satisfies::<&Position>(entity));
}

#[test]
fn buffer_multiple_commands_in_order() {
    let (mut world, reg) = setup();
    let e1 = world.spawn(());
    let e2 = world.spawn(());

    let mut buf = CommandBuffer::new();
    buf.add_default_by_name(e1, "Position");
    buf.add_default_by_name(e2, "Velocity");
    buf.remove_by_name(e1, "Position");
    buf.apply(&mut world, &reg).unwrap();

    assert!(!world.satisfies::<&Position>(e1));
    assert!(world.satisfies::<&Velocity>(e2));
}

#[test]
fn buffer_clear_discards_commands() {
    let mut buf = CommandBuffer::new();
    buf.spawn_empty();
    buf.clear();
    assert!(buf.is_empty());
}

#[test]
fn buffer_apply_unknown_component_in_bundle_errors() {
    let mut world = EcsWorld::new();
    let reg = ComponentRegistry::new(); // empty
    let entity = world.spawn(());

    let mut bundle = DynamicBundle::new();
    bundle.add(Position::default());

    let mut buf = CommandBuffer::new();
    buf.insert_bundle(entity, bundle);
    assert!(buf.apply(&mut world, &reg).is_err());
}
