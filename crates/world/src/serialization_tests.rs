//! Tests for world serialization data structures.

use crate::serialization::{SerializedEntity, WorldSnapshot, WorldSerializer, WorldDeserializer};
use std::collections::HashMap;

#[test]
fn serialized_entity_new() {
    let mut components = HashMap::new();
    components.insert("Position".to_string(), serde_json::json!({"x": 1.0}));
    let entity = SerializedEntity {
        id: 42,
        components,
        parent: Some(1),
    };
    assert_eq!(entity.id, 42);
    assert!(entity.components.contains_key("Position"));
    assert_eq!(entity.parent, Some(1));
}

#[test]
fn world_snapshot_new() {
    let snapshot = WorldSnapshot::new(1);
    assert_eq!(snapshot.version, 1);
    assert!(snapshot.entities.is_empty());
    assert!(snapshot.assets.is_empty());
}

#[test]
fn world_serializer_snapshot_returns_empty() {
    let serializer = WorldSerializer::new(2);
    let world = hecs::World::new();
    let snapshot = serializer.snapshot(&world);
    assert_eq!(snapshot.version, 2);
    assert!(snapshot.entities.is_empty());
}

#[test]
fn world_deserializer_load_does_not_panic() {
    let deserializer = WorldDeserializer;
    let snapshot = WorldSnapshot::new(1);
    let mut world = hecs::World::new();
    deserializer.load(&snapshot, &mut world);
    // Should not panic on empty snapshot
}
