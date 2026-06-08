//! Authority system for networked entities.
//!
//! Defines who controls an entity (server, a specific client, or
//! interpolated for remote players). The server enforces authority
//! boundaries: clients may only update entities they own, and the server
//! has the final say on all state.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::replication::NetworkId;
use crate::ClientId;

/// Who has authority over a replicated entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Authority {
    /// The server simulates and owns this entity exclusively.
    Server,
    /// A specific client predicts this entity locally.
    /// The server reconciles but does not override unless validation fails.
    Client(ClientId),
    /// This entity represents a remote player on the local client.
    /// It is interpolated between server snapshots, never predicted.
    Interpolated,
}

/// Component attached to replicated entities indicating their authority mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuthorityComponent {
    pub authority: Authority,
}

/// An authority transfer message sent by the server to change who owns
/// an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityTransfer {
    pub network_id: NetworkId,
    pub new_authority: Authority,
}

/// Server-side authority manager.
///
/// Tracks which client owns which entities and validates update requests.
#[derive(Debug, Clone, Default)]
pub struct AuthorityManager {
    /// Authority for each replicated entity.
    pub authorities: HashMap<NetworkId, Authority>,
    /// Set of entities that are currently predicted by a specific client.
    pub client_predicted: HashMap<ClientId, HashSet<NetworkId>>,
    /// Set of entities the server considers fully authoritative.
    pub server_authoritative: HashSet<NetworkId>,
}

impl AuthorityManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new entity and assign its initial authority.
    pub fn register(&mut self, network_id: NetworkId, authority: Authority) {
        self.authorities.insert(network_id, authority);
        match authority {
            Authority::Server => {
                self.server_authoritative.insert(network_id);
            }
            Authority::Client(client_id) => {
                self.client_predicted
                    .entry(client_id)
                    .or_default()
                    .insert(network_id);
            }
            Authority::Interpolated => {
                // Interpolated entities are not predicted by anyone locally.
            }
        }
    }

    /// Remove an entity from authority tracking (e.g. on despawn).
    pub fn unregister(&mut self, network_id: NetworkId) {
        if let Some(authority) = self.authorities.remove(&network_id) {
            match authority {
                Authority::Server => {
                    self.server_authoritative.remove(&network_id);
                }
                Authority::Client(client_id) => {
                    if let Some(set) = self.client_predicted.get_mut(&client_id) {
                        set.remove(&network_id);
                        if set.is_empty() {
                            self.client_predicted.remove(&client_id);
                        }
                    }
                }
                Authority::Interpolated => {}
            }
        }
    }

    /// Transfer authority of an entity to a new owner (server authority).
    pub fn transfer(&mut self, network_id: NetworkId, new_authority: Authority) -> Option<AuthorityTransfer> {
        let old = self.authorities.get(&network_id).copied()?;
        if old == new_authority {
            return None;
        }
        // Unregister from old bucket.
        self.unregister(network_id);
        // Re-register with new authority.
        self.register(network_id, new_authority);
        Some(AuthorityTransfer {
            network_id,
            new_authority,
        })
    }

    /// Check whether `client_id` is allowed to update `network_id`.
    pub fn can_client_update(&self, client_id: ClientId, network_id: NetworkId) -> bool {
        matches!(
            self.authorities.get(&network_id),
            Some(Authority::Client(owner)) if *owner == client_id
        )
    }

    /// Check whether the server owns this entity exclusively.
    pub fn is_server_authoritative(&self, network_id: NetworkId) -> bool {
        matches!(
            self.authorities.get(&network_id),
            Some(Authority::Server)
        )
    }

    /// Check whether this entity is interpolated on the client.
    pub fn is_interpolated(&self, network_id: NetworkId) -> bool {
        matches!(
            self.authorities.get(&network_id),
            Some(Authority::Interpolated)
        )
    }

    /// Get the authority for an entity, if any.
    pub fn get(&self, network_id: NetworkId) -> Option<Authority> {
        self.authorities.get(&network_id).copied()
    }

    /// Return all entities predicted by a given client.
    pub fn predicted_by(&self, client_id: ClientId) -> Option<&HashSet<NetworkId>> {
        self.client_predicted.get(&client_id)
    }

    /// Return all server-authoritative entities.
    pub fn server_entities(&self) -> &HashSet<NetworkId> {
        &self.server_authoritative
    }
}

/// Client-side authority manager.
///
/// The client mirrors the server's authority assignments for its own
/// entities and tracks which remote entities are interpolated.
#[derive(Debug, Clone, Default)]
pub struct ClientAuthorityManager {
    /// Authority for each known replicated entity.
    pub authorities: HashMap<NetworkId, Authority>,
    /// The client's own id (assigned during handshake).
    pub local_client_id: ClientId,
}

impl ClientAuthorityManager {
    pub fn new(local_client_id: ClientId) -> Self {
        Self {
            local_client_id,
            authorities: HashMap::new(),
        }
    }

    /// Apply an authority transfer from the server.
    pub fn apply_transfer(&mut self, transfer: &AuthorityTransfer) {
        self.authorities.insert(transfer.network_id, transfer.new_authority);
    }

    /// Set initial authority for a newly spawned entity.
    pub fn set_authority(&mut self, network_id: NetworkId, authority: Authority) {
        self.authorities.insert(network_id, authority);
    }

    /// Remove authority tracking for a despawned entity.
    pub fn remove(&mut self, network_id: NetworkId) {
        self.authorities.remove(&network_id);
    }

    /// Check whether this entity is locally predicted (owned by this client).
    pub fn is_local_predicted(&self, network_id: NetworkId) -> bool {
        matches!(
            self.authorities.get(&network_id),
            Some(Authority::Client(owner)) if *owner == self.local_client_id
        )
    }

    /// Check whether this entity is interpolated (remote player).
    pub fn is_interpolated(&self, network_id: NetworkId) -> bool {
        matches!(
            self.authorities.get(&network_id),
            Some(Authority::Interpolated)
        )
    }

    /// Check whether this entity is server-authoritative on the client
    /// (e.g. physics props, NPCs).
    pub fn is_server_authoritative(&self, network_id: NetworkId) -> bool {
        matches!(
            self.authorities.get(&network_id),
            Some(Authority::Server)
        )
    }

    /// Get the authority for an entity.
    pub fn get(&self, network_id: NetworkId) -> Option<Authority> {
        self.authorities.get(&network_id).copied()
    }

    /// Return all entities that are locally predicted.
    pub fn local_predicted_entities(&self) -> Vec<NetworkId> {
        self.authorities
            .iter()
            .filter(|(_, a)| matches!(a, Authority::Client(c) if *c == self.local_client_id))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Return all entities that are interpolated (remote players).
    pub fn interpolated_entities(&self) -> Vec<NetworkId> {
        self.authorities
            .iter()
            .filter(|(_, a)| matches!(a, Authority::Interpolated))
            .map(|(id, _)| *id)
            .collect()
    }
}
