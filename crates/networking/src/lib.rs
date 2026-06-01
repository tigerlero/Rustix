use serde::{Serialize, Deserialize};
use std::collections::VecDeque;

/// Unique identifier for a connected client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientId(pub u64);

/// Generic network message envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message<T> {
    pub from: ClientId,
    pub payload: T,
}

/// Events emitted by the network layer.
#[derive(Debug, Clone)]
pub enum NetworkEvent<T> {
    Connected(ClientId),
    Disconnected(ClientId),
    Message(Message<T>),
}

/// Simple in-memory network transport for local testing and LAN play.
/// Replace with QUIC/TCP implementation when ready.
#[derive(Debug, Clone)]
pub struct InMemoryTransport<T: Clone> {
    pub local_id: ClientId,
    pub outgoing: VecDeque<(ClientId, T)>,
    pub incoming: VecDeque<NetworkEvent<T>>,
    pub connected: bool,
}

impl<T: Clone> InMemoryTransport<T> {
    pub fn new(local_id: ClientId) -> Self {
        Self {
            local_id,
            outgoing: VecDeque::new(),
            incoming: VecDeque::new(),
            connected: false,
        }
    }

    pub fn connect(&mut self) {
        self.connected = true;
        self.incoming.push_back(NetworkEvent::Connected(self.local_id));
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        self.incoming.push_back(NetworkEvent::Disconnected(self.local_id));
    }

    pub fn send(&mut self, to: ClientId, payload: T) {
        self.outgoing.push_back((to, payload));
    }

    pub fn broadcast(&mut self, payload: T, peers: &[ClientId]) {
        for &peer in peers {
            if peer != self.local_id {
                self.outgoing.push_back((peer, payload.clone()));
            }
        }
    }

    pub fn receive(&mut self) -> Option<NetworkEvent<T>> {
        self.incoming.pop_front()
    }

    pub fn push_incoming(&mut self, event: NetworkEvent<T>) {
        self.incoming.push_back(event);
    }
}

/// Snapshot of entity state for network replication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub entity_id: u64,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

/// Server-authoritative snapshot of the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub tick: u64,
    pub entities: Vec<EntitySnapshot>,
}

/// Client input sent to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInput {
    pub tick: u64,
    pub move_dir: [f32; 3],
    pub jump: bool,
}

/// Network time synchronization packet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSync {
    pub server_time_ms: u64,
    pub client_time_ms: u64,
}
