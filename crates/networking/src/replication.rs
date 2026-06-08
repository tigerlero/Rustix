//! Networked ECS replication (spawn/despawn/update/destroy).
//!
//! This module provides the message types and replication protocol for
//! synchronizing entity state across a network. The caller is responsible
//! for mapping local `hecs::Entity` IDs to stable `NetworkId`s and for
//! serializing/deserializing component payloads using the engine's
//! component registry.

use rustix_core::ecs::{Entity, EcsWorld};
use serde::{Deserialize, Serialize};

/// Stable network identifier for a replicated entity.
///
/// Added as a component to any entity that should be replicated across
/// the network. The server assigns `NetworkId`s and the client mirrors
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetworkId(pub u64);

/// A component update for a single entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentUpdate {
    /// The replicated entity this update applies to.
    pub network_id: NetworkId,
    /// Human-readable component type name (used by the registry for dispatch).
    pub component_name: String,
    /// Serialized component payload.
    pub payload: Vec<u8>,
}

/// A full component removal for a single entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentRemoval {
    pub network_id: NetworkId,
    pub component_name: String,
}

/// A replicated entity spawn with its initial component bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnMessage {
    pub network_id: NetworkId,
    /// Serialized component name / payload pairs for the initial bundle.
    pub components: Vec<(String, Vec<u8>)>,
}

/// High-level replication messages exchanged between server and client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicationMessage {
    /// Server → Client: spawn a new replicated entity.
    Spawn(SpawnMessage),
    /// Server → Client: destroy a replicated entity.
    Despawn(NetworkId),
    /// Server → Client or Client → Server: update a component on an entity.
    Update(ComponentUpdate),
    /// Server → Client: remove a component from an entity.
    Remove(ComponentRemoval),
    /// Server → Client: batch of messages sent in one packet.
    Batch(Vec<ReplicationMessage>),
}

/// Tracks which entities and components changed locally so they can be
/// bundled into replication messages.
#[derive(Debug, Clone, Default)]
pub struct ReplicationTracker {
    pub spawned: Vec<NetworkId>,
    pub despawned: Vec<NetworkId>,
    pub updated: Vec<ComponentUpdate>,
    pub removed: Vec<ComponentRemoval>,
}

impl ReplicationTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.spawned.clear();
        self.despawned.clear();
        self.updated.clear();
        self.removed.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.spawned.is_empty()
            && self.despawned.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
    }

    /// Record a spawned entity.
    pub fn spawn(&mut self, network_id: NetworkId) {
        self.spawned.push(network_id);
    }

    /// Record a despawned entity.
    pub fn despawn(&mut self, network_id: NetworkId) {
        self.despawned.push(network_id);
    }

    /// Record a component update.
    pub fn update(&mut self, network_id: NetworkId, component_name: impl Into<String>, payload: Vec<u8>) {
        self.updated.push(ComponentUpdate {
            network_id,
            component_name: component_name.into(),
            payload,
        });
    }

    /// Record a component removal.
    pub fn remove(&mut self, network_id: NetworkId, component_name: impl Into<String>) {
        self.removed.push(ComponentRemoval {
            network_id,
            component_name: component_name.into(),
        });
    }

    /// Convert all tracked changes into a batch of `ReplicationMessage`s.
    ///
    /// Returns `None` if there is nothing to replicate.
    pub fn into_messages(self) -> Option<Vec<ReplicationMessage>> {
        if self.is_empty() {
            return None;
        }
        let mut messages = Vec::with_capacity(
            self.spawned.len()
                + self.despawned.len()
                + self.updated.len()
                + self.removed.len(),
        );
        // TODO: for spawns we need the full component bundle — this requires
        // the caller to supply the serialized components. For now, just
        // emit a placeholder Spawn that the caller can enrich.
        for id in self.spawned {
            messages.push(ReplicationMessage::Spawn(SpawnMessage {
                network_id: id,
                components: Vec::new(),
            }));
        }
        for id in self.despawned {
            messages.push(ReplicationMessage::Despawn(id));
        }
        for update in self.updated {
            messages.push(ReplicationMessage::Update(update));
        }
        for removal in self.removed {
            messages.push(ReplicationMessage::Remove(removal));
        }
        Some(messages)
    }
}

/// A client-side mapping from `NetworkId` to local `Entity`.
#[derive(Debug, Clone, Default)]
pub struct NetworkEntityMap {
    pub network_to_local: std::collections::HashMap<NetworkId, Entity>,
    pub local_to_network: std::collections::HashMap<Entity, NetworkId>,
}

impl NetworkEntityMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, network_id: NetworkId, local: Entity) {
        self.network_to_local.insert(network_id, local);
        self.local_to_network.insert(local, network_id);
    }

    pub fn remove_by_network(&mut self, network_id: NetworkId) -> Option<Entity> {
        let local = self.network_to_local.remove(&network_id)?;
        self.local_to_network.remove(&local);
        Some(local)
    }

    pub fn remove_by_local(&mut self, local: Entity) -> Option<NetworkId> {
        let network_id = self.local_to_network.remove(&local)?;
        self.network_to_local.remove(&network_id);
        Some(network_id)
    }

    pub fn get_local(&self, network_id: NetworkId) -> Option<Entity> {
        self.network_to_local.get(&network_id).copied()
    }

    pub fn get_network(&self, local: Entity) -> Option<NetworkId> {
        self.local_to_network.get(&local).copied()
    }

    pub fn clear(&mut self) {
        self.network_to_local.clear();
        self.local_to_network.clear();
    }
}

/// Trait for systems that can serialize and deserialize component data
/// using the engine's component registry.
///
/// The runtime implements this trait and passes it to the replication
/// system so that the networking crate stays decoupled from the ECS
/// registry internals.
pub trait ComponentSerializer {
    /// Serialize a component from `entity` in `world`.
    ///
    /// Returns `None` if the entity does not have the component.
    fn serialize_component(
        &self,
        world: &EcsWorld,
        entity: Entity,
        component_name: &str,
    ) -> Option<Vec<u8>>;

    /// Deserialize `payload` into a component and insert it into `entity`.
    fn deserialize_component(
        &self,
        world: &mut EcsWorld,
        entity: Entity,
        component_name: &str,
        payload: &[u8],
    ) -> Result<(), String>;

    /// Remove a component from `entity` by name.
    fn remove_component(
        &self,
        world: &mut EcsWorld,
        entity: Entity,
        component_name: &str,
    ) -> Result<(), String>;

    /// Build an initial component bundle for a spawned entity.
    fn spawn_bundle(
        &self,
        world: &mut EcsWorld,
        entity: Entity,
        components: &[(String, Vec<u8>)],
    ) -> Result<(), String>;
}

/// Apply a single `ReplicationMessage` to the local ECS world.
///
/// `map` tracks the network-id ↔ local-entity mapping and is updated
/// for spawn/despawn operations.
///
/// `serializer` handles the actual component serialization/deserialization.
pub fn apply_replication_message(
    world: &mut EcsWorld,
    map: &mut NetworkEntityMap,
    serializer: &dyn ComponentSerializer,
    message: &ReplicationMessage,
) -> Result<(), String> {
    match message {
        ReplicationMessage::Spawn(spawn) => {
            let local = world.spawn(());
            serializer.spawn_bundle(world, local, &spawn.components)?;
            map.insert(spawn.network_id, local);
            Ok(())
        }
        ReplicationMessage::Despawn(network_id) => {
            if let Some(local) = map.remove_by_network(*network_id) {
                let _ = world.despawn(local);
            }
            Ok(())
        }
        ReplicationMessage::Update(update) => {
            if let Some(local) = map.get_local(update.network_id) {
                serializer.deserialize_component(
                    world,
                    local,
                    &update.component_name,
                    &update.payload,
                )?;
            }
            Ok(())
        }
        ReplicationMessage::Remove(removal) => {
            if let Some(local) = map.get_local(removal.network_id) {
                serializer.remove_component(world, local, &removal.component_name)?;
            }
            Ok(())
        }
        ReplicationMessage::Batch(batch) => {
            for msg in batch {
                apply_replication_message(world, map, serializer, msg)?;
            }
            Ok(())
        }
    }
}

/// Collect all `ReplicationMessage`s into a single `Batch` if there are
/// multiple items, otherwise return the single message directly.
pub fn batch_messages(messages: Vec<ReplicationMessage>) -> ReplicationMessage {
    if messages.len() == 1 {
        messages.into_iter().next().unwrap()
    } else {
        ReplicationMessage::Batch(messages)
    }
}
