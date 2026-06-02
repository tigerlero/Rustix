use super::*;
use crate::ecs::EcsWorld;

// ---------------------------------------------------------------------------
// Test component types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ComponentRegistry tests
// ---------------------------------------------------------------------------

#[test]
fn registry_starts_empty() {
    let reg = ComponentRegistry::new();
    assert!(reg.is_empty());
    assert_eq!(reg.len(), 0);
}

#[test]
fn registry_can_register_component() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    assert_eq!(reg.len(), 1);
    assert!(!reg.is_empty());
}

#[test]
fn registry_look_up_by_type_id() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap();
    assert_eq!(info.type_id, TypeId::of::<Position>());
    assert_eq!(info.size, std::mem::size_of::<Position>());
    assert_eq!(info.align, std::mem::align_of::<Position>());
}

#[test]
fn registry_look_up_by_name() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_name("Position").unwrap();
    assert_eq!(info.type_id, TypeId::of::<Position>());
}

#[test]
fn registry_name_lookup_fails_for_unknown() {
    let reg = ComponentRegistry::new();
    assert!(reg.get_by_name("NonExistent").is_none());
}

#[test]
fn registry_type_id_lookup_fails_for_unknown() {
    let reg = ComponentRegistry::new();
    assert!(reg.get_by_type_id(TypeId::of::<Position>()).is_none());
}

#[test]
fn registry_default_value_produces_correct_type() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap();
    let boxed = info.default_value();
    let pos = boxed.downcast_ref::<Position>().unwrap();
    assert_eq!(pos, &Position::default());
}

#[test]
fn registry_clone_to_boxed_produces_equal_value() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap();
    let original = Position { x: 1.0, y: 2.0, z: 3.0 };
    let boxed = unsafe { info.clone_to_boxed(&original as *const _ as *const u8) };
    let cloned = boxed.downcast_ref::<Position>().unwrap();
    assert_eq!(cloned, &original);
}

#[test]
fn registry_iterates_all_registered() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    reg.register::<Velocity>();
    let names: Vec<_> = reg.iter().map(|i| i.name).collect();
    assert!(names.contains(&"Position"));
    assert!(names.contains(&"Velocity"));
}

#[test]
fn registry_type_id_by_name() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let id = reg.type_id_by_name("Position").unwrap();
    assert_eq!(id, TypeId::of::<Position>());
}

// ---------------------------------------------------------------------------
// ErasedStorage tests
// ---------------------------------------------------------------------------

fn make_entity() -> Entity {
    thread_local! {
        static WORLD: std::cell::RefCell<EcsWorld> = std::cell::RefCell::new(EcsWorld::new());
    }
    WORLD.with(|w| w.borrow_mut().spawn(()))
}

#[test]
fn erased_storage_insert_and_get() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let mut storage = ErasedStorage::new(info);
    let pos = Position { x: 10.0, y: 20.0, z: 30.0 };
    let entity = make_entity();

    storage.insert(entity, &pos as *const _ as *const u8);

    let raw = storage.get(entity).unwrap();
    let retrieved = unsafe { (raw as *mut Position).as_ref().unwrap() };
    assert_eq!(retrieved, &pos);
}

#[test]
fn erased_storage_overwrite_existing() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let mut storage = ErasedStorage::new(info);
    let entity = make_entity();

    storage.insert(entity, &Position { x: 1.0, y: 0.0, z: 0.0 } as *const _ as *const u8);
    storage.insert(entity, &Position { x: 99.0, y: 0.0, z: 0.0 } as *const _ as *const u8);

    let raw = storage.get(entity).unwrap();
    let retrieved = unsafe { (raw as *mut Position).as_ref().unwrap() };
    assert_eq!(retrieved.x, 99.0);
}

#[test]
fn erased_storage_remove_existing() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let mut storage = ErasedStorage::new(info);
    let entity = make_entity();
    storage.insert(entity, &Position::default() as *const _ as *const u8);

    assert!(storage.remove(entity));
    assert!(storage.get(entity).is_none());
    assert!(storage.is_empty());
}

#[test]
fn erased_storage_remove_nonexistent() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let mut storage = ErasedStorage::new(info);
    let entity = make_entity();
    assert!(!storage.remove(entity));
}

#[test]
fn erased_storage_multiple_entities() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let mut storage = ErasedStorage::new(info);
    let e1 = make_entity();
    let e2 = make_entity();

    storage.insert(e1, &Position { x: 1.0, y: 0.0, z: 0.0 } as *const _ as *const u8);
    storage.insert(e2, &Position { x: 2.0, y: 0.0, z: 0.0 } as *const _ as *const u8);

    assert_eq!(storage.len(), 2);

    let r1 = unsafe { (storage.get(e1).unwrap() as *mut Position).as_ref().unwrap() };
    let r2 = unsafe { (storage.get(e2).unwrap() as *mut Position).as_ref().unwrap() };
    assert_eq!(r1.x, 1.0);
    assert_eq!(r2.x, 2.0);
}

#[test]
fn erased_storage_swap_remove_preserves_other_entity() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let mut storage = ErasedStorage::new(info);
    let e1 = make_entity();
    let e2 = make_entity();

    storage.insert(e1, &Position { x: 1.0, y: 0.0, z: 0.0 } as *const _ as *const u8);
    storage.insert(e2, &Position { x: 2.0, y: 0.0, z: 0.0 } as *const _ as *const u8);

    storage.remove(e1);

    assert!(storage.get(e1).is_none());
    let r2 = unsafe { (storage.get(e2).unwrap() as *mut Position).as_ref().unwrap() };
    assert_eq!(r2.x, 2.0);
    assert_eq!(storage.len(), 1);
}

#[test]
fn erased_storage_clone_value_roundtrip() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let mut storage = ErasedStorage::new(info);
    let entity = make_entity();
    let original = Position { x: 3.14, y: 2.71, z: 1.41 };
    storage.insert(entity, &original as *const _ as *const u8);

    let boxed = storage.clone_value(entity).unwrap();
    let cloned = boxed.downcast_ref::<Position>().unwrap();
    assert_eq!(cloned, &original);
}

#[test]
fn erased_storage_clone_value_nonexistent() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let storage = ErasedStorage::new(info);
    let entity = make_entity();
    assert!(storage.clone_value(entity).is_none());
}

#[test]
fn erased_storage_iter_yields_all() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let mut storage = ErasedStorage::new(info);
    let e1 = make_entity();
    let e2 = make_entity();

    storage.insert(e1, &Position { x: 1.0, y: 0.0, z: 0.0 } as *const _ as *const u8);
    storage.insert(e2, &Position { x: 2.0, y: 0.0, z: 0.0 } as *const _ as *const u8);

    let mut xs = Vec::new();
    for (_e, ptr) in storage.iter() {
        let pos = unsafe { (ptr as *mut Position).as_ref().unwrap() };
        xs.push(pos.x);
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(xs, vec![1.0, 2.0]);
}

// ---------------------------------------------------------------------------
// ErasedWorld tests
// ---------------------------------------------------------------------------

#[test]
fn erased_world_insert_and_get() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let mut world = ErasedWorld::default();
    let entity = make_entity();
    let pos = Position { x: 5.0, y: 6.0, z: 7.0 };

    world.insert(&info, entity, &pos as *const _ as *const u8);

    let raw = world.get(TypeId::of::<Position>(), entity).unwrap();
    let retrieved = unsafe { (raw as *mut Position).as_ref().unwrap() };
    assert_eq!(retrieved, &pos);
}

#[test]
fn erased_world_remove_existing() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();

    let mut world = ErasedWorld::default();
    let entity = make_entity();
    world.insert(&info, entity, &Position::default() as *const _ as *const u8);

    assert!(world.remove(TypeId::of::<Position>(), entity));
    assert!(world.get(TypeId::of::<Position>(), entity).is_none());
}

#[test]
fn erased_world_remove_nonexistent_type() {
    let mut world = ErasedWorld::default();
    let entity = make_entity();
    assert!(!world.remove(TypeId::of::<Position>(), entity));
}

#[test]
fn erased_world_storage_count_tracks_types() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    reg.register::<Velocity>();
    let pos_info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap().clone();
    let vel_info = reg.get_by_type_id(TypeId::of::<Velocity>()).unwrap().clone();

    let mut world = ErasedWorld::default();
    let entity = make_entity();

    world.insert(&pos_info, entity, &Position::default() as *const _ as *const u8);
    assert_eq!(world.storage_count(), 1);

    world.insert(&vel_info, entity, &Velocity::default() as *const _ as *const u8);
    assert_eq!(world.storage_count(), 2);
}

// ---------------------------------------------------------------------------
// Drop safety tests
// ---------------------------------------------------------------------------

#[test]
fn erased_storage_drop_runs_for_all_elements() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Clone)]
    struct DropCounter;
    impl Drop for DropCounter {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }
    impl Default for DropCounter {
        fn default() -> Self { Self }
    }

    // Pre-create values so their temporaries don't count.
    let dc1 = DropCounter;
    let dc2 = DropCounter;
    DROP_COUNT.store(0, Ordering::SeqCst);

    {
        let mut reg = ComponentRegistry::new();
        reg.register::<DropCounter>();
        let info = reg.get_by_type_id(TypeId::of::<DropCounter>()).unwrap().clone();

        let mut storage = ErasedStorage::new(info);
        let e1 = make_entity();
        let e2 = make_entity();
        storage.insert(e1, &dc1 as *const _ as *const u8);
        storage.insert(e2, &dc2 as *const _ as *const u8);
        storage.remove(e1);
        // e1 dropped explicitly via remove (1), e2 still alive
    }

    // e2 dropped when storage goes out of scope (1) → total 2
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 2);
}

// ---------------------------------------------------------------------------
// DynamicBundle tests
// ---------------------------------------------------------------------------

#[test]
fn dynamic_bundle_add_typed_components() {
    let mut bundle = DynamicBundle::new();
    bundle.add(Position { x: 1.0, y: 2.0, z: 3.0 });
    bundle.add(Velocity { dx: 4.0, dy: 5.0 });
    assert_eq!(bundle.len(), 2);
    assert!(!bundle.is_empty());
}

#[test]
fn dynamic_bundle_add_erased_components() {
    let mut bundle = DynamicBundle::new();
    bundle.add_erased(TypeId::of::<Position>(), Box::new(Position { x: 1.0, y: 2.0, z: 3.0 }));
    assert_eq!(bundle.len(), 1);
}

#[test]
fn registry_add_component_by_name() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();

    let mut world = EcsWorld::new();
    let entity = world.spawn(());

    reg.add_component_by_name(&mut world, entity, "Position").unwrap();

    let count = world.query_mut::<&Position>().into_iter().count();
    assert_eq!(count, 1);
}

#[test]
fn registry_add_component_by_name_unknown_fails() {
    let reg = ComponentRegistry::new();
    let mut world = EcsWorld::new();
    let entity = world.spawn(());
    assert!(reg.add_component_by_name(&mut world, entity, "Ghost").is_err());
}

#[test]
fn registry_remove_component_by_name() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();

    let mut world = EcsWorld::new();
    let entity = world.spawn((Position { x: 1.0, y: 2.0, z: 3.0 },));

    let removed = reg.remove_component_by_name(&mut world, entity, "Position").unwrap();
    assert!(removed.is_some());

    let count = world.query_mut::<&Position>().into_iter().count();
    assert_eq!(count, 0);
}

#[test]
fn registry_insert_bundle_consumes_and_inserts() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    reg.register::<Velocity>();

    let mut bundle = DynamicBundle::new();
    bundle.add(Position { x: 1.0, y: 2.0, z: 3.0 });
    bundle.add(Velocity { dx: 10.0, dy: 20.0 });

    let mut world = EcsWorld::new();
    let entity = world.spawn(());

    reg.insert_bundle(&mut world, entity, bundle).unwrap();

    assert_eq!(world.query_mut::<&Position>().into_iter().count(), 1);
    assert_eq!(world.query_mut::<&Velocity>().into_iter().count(), 1);
}

#[test]
fn registry_insert_bundle_unknown_type_fails() {
    let reg = ComponentRegistry::new();
    let mut bundle = DynamicBundle::new();
    bundle.add_erased(TypeId::of::<Position>(), Box::new(Position::default()));

    let mut world = EcsWorld::new();
    let entity = world.spawn(());

    assert!(reg.insert_bundle(&mut world, entity, bundle).is_err());
}

#[test]
fn component_info_insert_into_world_and_remove_from_world() {
    let mut reg = ComponentRegistry::new();
    reg.register::<Position>();
    let info = reg.get_by_type_id(TypeId::of::<Position>()).unwrap();

    let mut world = EcsWorld::new();
    let entity = world.spawn(());

    info.insert_into_world(&mut world, entity, Box::new(Position { x: 7.0, y: 8.0, z: 9.0 }));
    assert!(world.satisfies::<&Position>(entity));

    let removed = info.remove_from_world(&mut world, entity);
    assert!(removed.is_some());
    assert!(!world.satisfies::<&Position>(entity));
}
