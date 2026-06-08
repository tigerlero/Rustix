//! Bandwidth optimization: delta compression and interest management.
//!
//! Reduces the amount of data sent over the network by:
//! 1. **Delta compression** — only replicating component fields that changed
//!    since the last acknowledged state for each client.
//! 2. **Interest management** — only sending updates for entities that a
//!    client actually cares about (e.g. within a spatial radius).

use std::collections::{HashMap, HashSet};

use crate::replication::{ComponentUpdate, NetworkId, ReplicationMessage};
use crate::ClientId;

/// Per-client, per-entity tracking of the last replicated state hash.
///
/// Used by the server to decide whether a component update needs to be
/// sent to a specific client.
#[derive(Debug, Clone, Default)]
pub struct DeltaCompressor {
    /// Maps `(ClientId, NetworkId, component_name)` → last known hash.
    last_hashes: HashMap<(ClientId, NetworkId, String), u64>,
}

impl DeltaCompressor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a client so that we can track per-client last-known state.
    pub fn register_client(&mut self, _client_id: ClientId) {
        // No-op; state is allocated on first `record_sent`.
    }

    /// Remove all tracked state for a disconnected client.
    pub fn unregister_client(&mut self, client_id: ClientId) {
        self.last_hashes
            .retain(|(cid, _, _), _| *cid != client_id);
    }

    /// Remove all tracked state for a despawned entity.
    pub fn unregister_entity(&mut self, network_id: NetworkId) {
        self.last_hashes
            .retain(|(_, nid, _), _| *nid != network_id);
    }

    /// Check whether `payload` for `component_name` on `network_id` has
    /// changed relative to the last value sent to `client_id`.
    ///
    /// Returns `true` if the data is new or the client has never seen it.
    pub fn is_changed(&self, client_id: ClientId, network_id: NetworkId, component_name: &str, payload: &[u8]) -> bool {
        let key = (client_id, network_id, component_name.to_string());
        let hash = fxhash::hash64(payload);
        match self.last_hashes.get(&key) {
            Some(last) => *last != hash,
            None => true,
        }
    }

    /// Record that `payload` for `component_name` on `network_id` was
    /// just sent to `client_id`.
    pub fn record_sent(&mut self, client_id: ClientId, network_id: NetworkId, component_name: &str, payload: &[u8]) {
        let key = (client_id, network_id, component_name.to_string());
        let hash = fxhash::hash64(payload);
        self.last_hashes.insert(key, hash);
    }

    /// Filter a list of `ComponentUpdate`s, returning only those that are
    /// actually different from the last-known state for `client_id`.
    pub fn filter_updates(
        &self,
        client_id: ClientId,
        updates: Vec<ComponentUpdate>,
    ) -> Vec<ComponentUpdate> {
        updates
            .into_iter()
            .filter(|u| self.is_changed(client_id, u.network_id, &u.component_name, &u.payload))
            .collect()
    }

    /// Record that a batch of updates was sent to `client_id`.
    pub fn record_batch(&mut self, client_id: ClientId, updates: &[ComponentUpdate]) {
        for u in updates {
            self.record_sent(client_id, u.network_id, &u.component_name, &u.payload);
        }
    }

    /// Number of tracked `(client, entity, component)` entries.
    pub fn len(&self) -> usize {
        self.last_hashes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.last_hashes.is_empty()
    }

    /// Clear all tracked state.
    pub fn clear(&mut self) {
        self.last_hashes.clear();
    }
}

/// A fast 64-bit hash function suitable for change-detection.
mod fxhash {
    #[inline]
    pub fn hash64(bytes: &[u8]) -> u64 {
        // FNV-1a 64-bit
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x00000100000001b3;
        let mut hash = FNV_OFFSET;
        for &b in bytes {
            hash ^= b as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }
}

/// Criteria that determine whether a client is interested in an entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterestCriteria {
    /// Maximum squared distance from the client observer.
    pub max_distance_sq: f32,
    /// Whether to include entities with `Authority::Server` unconditionally.
    pub include_server_authoritative: bool,
    /// Whether to always include the client's own predicted entities.
    pub always_include_own: bool,
}

impl Default for InterestCriteria {
    fn default() -> Self {
        Self {
            max_distance_sq: f32::INFINITY,
            include_server_authoritative: true,
            always_include_own: true,
        }
    }
}

/// Per-client interest set: which entities should be replicated to this
/// client.
#[derive(Debug, Clone, Default)]
pub struct InterestManager {
    /// Client-specific criteria.
    criteria: HashMap<ClientId, InterestCriteria>,
    /// Cached per-client interest sets.
    interest_sets: HashMap<ClientId, HashSet<NetworkId>>,
}

impl InterestManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set or update the criteria for a client.
    pub fn set_criteria(&mut self, client_id: ClientId, criteria: InterestCriteria) {
        self.criteria.insert(client_id, criteria);
    }

    /// Register a client (allocates empty interest set).
    pub fn register_client(&mut self, client_id: ClientId) {
        self.interest_sets.entry(client_id).or_default();
    }

    /// Remove all data for a disconnected client.
    pub fn unregister_client(&mut self, client_id: ClientId) {
        self.criteria.remove(&client_id);
        self.interest_sets.remove(&client_id);
    }

    /// Update the interest set for `client_id` based on the provided
    /// `(NetworkId, position)` list.
    ///
    /// `client_position` is the observer's current position (e.g. their
    /// controlled entity's transform).
    pub fn update_interest_set(
        &mut self,
        client_id: ClientId,
        client_position: [f32; 3],
        entities: &[(NetworkId, [f32; 3])],
        own_entities: &[NetworkId],
    ) {
        let criteria = self.criteria.get(&client_id).copied().unwrap_or_default();
        let mut set = HashSet::with_capacity(entities.len());

        for (network_id, pos) in entities {
            let dx = pos[0] - client_position[0];
            let dy = pos[1] - client_position[1];
            let dz = pos[2] - client_position[2];
            let dist_sq = dx * dx + dy * dy + dz * dz;

            if dist_sq <= criteria.max_distance_sq {
                set.insert(*network_id);
            }
        }

        if criteria.always_include_own {
            for &id in own_entities {
                set.insert(id);
            }
        }

        self.interest_sets.insert(client_id, set);
    }

    /// Check whether `client_id` should receive updates for `network_id`.
    pub fn is_interested(&self, client_id: ClientId, network_id: NetworkId) -> bool {
        self.interest_sets
            .get(&client_id)
            .map(|set| set.contains(&network_id))
            .unwrap_or(false)
    }

    /// Filter a list of `ReplicationMessage`s, keeping only those for
    /// entities the client is interested in.
    pub fn filter_messages(
        &self,
        client_id: ClientId,
        messages: Vec<ReplicationMessage>,
    ) -> Vec<ReplicationMessage> {
        let interested = match self.interest_sets.get(&client_id) {
            Some(set) => set,
            None => return Vec::new(),
        };

        messages
            .into_iter()
            .filter(|msg| network_id_from_message(msg).map_or(false, |nid| interested.contains(&nid)))
            .collect()
    }

    /// Return the current interest set for a client.
    pub fn interest_set(&self, client_id: ClientId) -> Option<&HashSet<NetworkId>> {
        self.interest_sets.get(&client_id)
    }

    /// Number of tracked clients.
    pub fn client_count(&self) -> usize {
        self.interest_sets.len()
    }

    /// Clear all interest data.
    pub fn clear(&mut self) {
        self.criteria.clear();
        self.interest_sets.clear();
    }
}

/// Extract the `NetworkId` from a `ReplicationMessage`, if any.
fn network_id_from_message(msg: &ReplicationMessage) -> Option<NetworkId> {
    match msg {
        ReplicationMessage::Spawn(s) => Some(s.network_id),
        ReplicationMessage::Despawn(nid) => Some(*nid),
        ReplicationMessage::Update(u) => Some(u.network_id),
        ReplicationMessage::Remove(r) => Some(r.network_id),
        ReplicationMessage::Batch(_) => None,
    }
}

/// Combined bandwidth optimizer that applies both delta compression and
/// interest management in one pass.
#[derive(Debug, Clone, Default)]
pub struct BandwidthOptimizer {
    pub delta: DeltaCompressor,
    pub interest: InterestManager,
}

impl BandwidthOptimizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new connected client.
    pub fn register_client(&mut self, client_id: ClientId) {
        self.delta.register_client(client_id);
        self.interest.register_client(client_id);
    }

    /// Clean up data for a disconnected client.
    pub fn unregister_client(&mut self, client_id: ClientId) {
        self.delta.unregister_client(client_id);
        self.interest.unregister_client(client_id);
    }

    /// Clean up data for a despawned entity.
    pub fn unregister_entity(&mut self, network_id: NetworkId) {
        self.delta.unregister_entity(network_id);
    }

    /// Process a batch of `ReplicationMessage`s for a specific client.
    ///
    /// Steps:
    /// 1. Filter out messages for entities the client is not interested in.
    /// 2. For `Update` messages, remove unchanged component payloads.
    /// 3. Record the sent state so future calls know what changed.
    pub fn optimize_for_client(
        &mut self,
        client_id: ClientId,
        messages: Vec<ReplicationMessage>,
    ) -> Vec<ReplicationMessage> {
        let interested = self.interest.filter_messages(client_id, messages);
        let mut optimized = Vec::with_capacity(interested.len());

        for msg in interested {
            match msg {
                ReplicationMessage::Update(update) => {
                    if self.delta.is_changed(
                        client_id,
                        update.network_id,
                        &update.component_name,
                        &update.payload,
                    ) {
                        self.delta.record_sent(
                            client_id,
                            update.network_id,
                            &update.component_name,
                            &update.payload,
                        );
                        optimized.push(ReplicationMessage::Update(update));
                    }
                }
                other => {
                    optimized.push(other);
                }
            }
        }

        optimized
    }
}
