//! World serialization (`.rxworld` format).
//!
//! Provides structures for serializing entity layouts, component
//! data, and asset references to a format that can be loaded back.

use hecs::Entity;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Serialized representation of a single entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedEntity {
    pub id: u64,
    pub components: HashMap<String, serde_json::Value>,
    pub parent: Option<u64>,
}

/// Serialized world snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub version: u32,
    pub entities: Vec<SerializedEntity>,
    pub assets: Vec<String>,
}

impl WorldSnapshot {
    pub fn new(version: u32) -> Self {
        Self {
            version,
            entities: Vec::new(),
            assets: Vec::new(),
        }
    }
}

/// A serializer that produces `WorldSnapshot`s from a `hecs::World`.
///
/// Concrete component serialization is left to the caller via
/// `register_component<T>(name)`.
pub struct WorldSerializer {
    version: u32,
}

impl WorldSerializer {
    pub fn new(version: u32) -> Self {
        Self { version }
    }

    pub fn snapshot(&self, _world: &hecs::World) -> WorldSnapshot {
        // Stub: real implementation would iterate ECS archetypes,
        // serialize each component via a type registry, and build
        // parent-child mappings from scene-graph components.
        WorldSnapshot::new(self.version)
    }
}

/// Deserializer that rebuilds a `hecs::World` from a `WorldSnapshot`.
pub struct WorldDeserializer;

impl WorldDeserializer {
    pub fn load(&self, _snapshot: &WorldSnapshot, _world: &mut hecs::World) {
        // Stub: real implementation would spawn entities, insert
        // deserialized components, and re-link parent-child edges.
    }
}
