//! Connection-oriented protocol on top of UDP.
//!
//! Handles handshake, heartbeat, disconnect, and message sequencing.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, Instant};

/// Connection state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Handshaking,
    Connected,
    Disconnecting,
    Disconnected,
}

/// Header for every protocol packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// Client → Server: initial handshake request.
    HandshakeRequest,
    /// Server → Client: handshake accepted, assigned client id.
    HandshakeResponse,
    /// Bidirectional: keep-alive heartbeat.
    Heartbeat,
    /// Bidirectional: ordered reliable payload.
    Reliable,
    /// Bidirectional: unordered unreliable payload (e.g. snapshots, inputs).
    Unreliable,
    /// Bidirectional: graceful disconnect.
    Disconnect,
}

impl PacketType {
    pub fn to_u8(self) -> u8 {
        match self {
            PacketType::HandshakeRequest => 0,
            PacketType::HandshakeResponse => 1,
            PacketType::Heartbeat => 2,
            PacketType::Reliable => 3,
            PacketType::Unreliable => 4,
            PacketType::Disconnect => 5,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(PacketType::HandshakeRequest),
            1 => Some(PacketType::HandshakeResponse),
            2 => Some(PacketType::Heartbeat),
            3 => Some(PacketType::Reliable),
            4 => Some(PacketType::Unreliable),
            5 => Some(PacketType::Disconnect),
            _ => None,
        }
    }
}

/// Low-level protocol packet.
#[derive(Debug, Clone)]
pub struct ProtocolPacket {
    pub packet_type: PacketType,
    /// Monotonically increasing sequence number for reliable ordering.
    pub sequence: u16,
    /// Ack field for reliable messages (simple sliding window ack).
    pub ack: u16,
    pub payload: Vec<u8>,
}

impl ProtocolPacket {
    /// Minimal wire format: `[type:1][sequence:2][ack:2][payload:..]`
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5 + self.payload.len());
        buf.push(self.packet_type.to_u8());
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.extend_from_slice(&self.ack.to_le_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }
        let packet_type = PacketType::from_u8(data[0])?;
        let sequence = u16::from_le_bytes([data[1], data[2]]);
        let ack = u16::from_le_bytes([data[3], data[4]]);
        let payload = data[5..].to_vec();
        Some(Self {
            packet_type,
            sequence,
            ack,
            payload,
        })
    }
}

/// A virtual connection over UDP with reliability, ordering, and heartbeat.
pub struct VirtualConnection {
    pub addr: SocketAddr,
    pub state: ConnectionState,
    pub client_id: u64,
    /// Next outgoing sequence number for reliable messages.
    pub next_sequence: u16,
    /// Last received sequence number from the peer.
    pub last_received_seq: u16,
    /// Messages waiting to be acknowledged.
    pub pending_ack: VecDeque<(u16, Instant, Vec<u8>)>,
    /// Ordered reliable incoming messages ready for consumption.
    pub reliable_inbox: VecDeque<Vec<u8>>,
    /// Unreliable incoming messages ready for consumption.
    pub unreliable_inbox: VecDeque<Vec<u8>>,
    pub last_heartbeat: Instant,
    pub heartbeat_interval: Duration,
    pub disconnect_timeout: Duration,
}

impl VirtualConnection {
    pub fn new(addr: SocketAddr, client_id: u64) -> Self {
        Self {
            addr,
            state: ConnectionState::Handshaking,
            client_id,
            next_sequence: 1,
            last_received_seq: 0,
            pending_ack: VecDeque::new(),
            reliable_inbox: VecDeque::new(),
            unreliable_inbox: VecDeque::new(),
            last_heartbeat: Instant::now(),
            heartbeat_interval: Duration::from_secs(1),
            disconnect_timeout: Duration::from_secs(5),
        }
    }

    /// Queue a reliable ordered message for sending.
    pub fn send_reliable(&mut self, payload: Vec<u8>) -> ProtocolPacket {
        let seq = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.pending_ack.push_back((seq, Instant::now(), payload.clone()));
        ProtocolPacket {
            packet_type: PacketType::Reliable,
            sequence: seq,
            ack: self.last_received_seq,
            payload,
        }
    }

    /// Create an unreliable packet (no ack, no ordering).
    pub fn send_unreliable(&self, payload: Vec<u8>) -> ProtocolPacket {
        ProtocolPacket {
            packet_type: PacketType::Unreliable,
            sequence: 0,
            ack: self.last_received_seq,
            payload,
        }
    }

    /// Process an incoming packet from the peer.
    pub fn receive_packet(&mut self, packet: ProtocolPacket) {
        self.last_heartbeat = Instant::now();

        match packet.packet_type {
            PacketType::HandshakeRequest => {
                // Server receives this.
                if self.state == ConnectionState::Handshaking {
                    self.state = ConnectionState::Connected;
                }
            }
            PacketType::HandshakeResponse => {
                // Client receives this.
                if self.state == ConnectionState::Handshaking {
                    self.state = ConnectionState::Connected;
                }
            }
            PacketType::Heartbeat => {}
            PacketType::Reliable => {
                if packet.sequence > self.last_received_seq {
                    self.last_received_seq = packet.sequence;
                    self.reliable_inbox.push_back(packet.payload);
                }
                // If sequence <= last_received_seq it's a duplicate — ignore.
            }
            PacketType::Unreliable => {
                self.unreliable_inbox.push_back(packet.payload);
            }
            PacketType::Disconnect => {
                self.state = ConnectionState::Disconnected;
            }
        }
    }

    /// Returns `true` if the connection has timed out (no heartbeat).
    pub fn is_timed_out(&self) -> bool {
        self.last_heartbeat.elapsed() > self.disconnect_timeout
    }

    /// Returns packets that need retransmission (reliable messages not acked).
    pub fn pending_retransmits(&self, timeout: Duration) -> Vec<ProtocolPacket> {
        let now = Instant::now();
        self.pending_ack
            .iter()
            .filter(|(_, sent, _)| now.duration_since(*sent) > timeout)
            .map(|(seq, _, payload)| ProtocolPacket {
                packet_type: PacketType::Reliable,
                sequence: *seq,
                ack: self.last_received_seq,
                payload: payload.clone(),
            })
            .collect()
    }

    /// Acknowledge receipt of sequence `seq` by removing from pending queue.
    pub fn ack(&mut self, seq: u16) {
        self.pending_ack.retain(|(s, _, _)| *s != seq);
    }

    /// Create a heartbeat packet.
    pub fn heartbeat_packet(&self) -> ProtocolPacket {
        ProtocolPacket {
            packet_type: PacketType::Heartbeat,
            sequence: 0,
            ack: self.last_received_seq,
            payload: Vec::new(),
        }
    }
}

/// Manages multiple virtual connections on a shared UDP socket.
pub struct ConnectionManager {
    pub socket: Arc<UdpSocket>,
    pub connections: std::collections::HashMap<SocketAddr, VirtualConnection>,
    pub next_client_id: u64,
}

impl ConnectionManager {
    pub fn new(socket: Arc<UdpSocket>) -> Self {
        Self {
            socket,
            connections: std::collections::HashMap::new(),
            next_client_id: 1,
        }
    }

    /// Handle an incoming raw UDP packet.
    pub fn handle_packet(&mut self, addr: SocketAddr, data: &[u8]) {
        let Some(packet) = ProtocolPacket::decode(data) else {
            tracing::warn!("Failed to decode packet from {}", addr);
            return;
        };

        match packet.packet_type {
            PacketType::HandshakeRequest => {
                // New connection — assign client id.
                let client_id = self.next_client_id;
                self.next_client_id += 1;
                let mut conn = VirtualConnection::new(addr, client_id);
                conn.receive_packet(packet);
                self.connections.insert(addr, conn);
            }
            _ => {
                if let Some(conn) = self.connections.get_mut(&addr) {
                    conn.receive_packet(packet);
                }
            }
        }
    }

    /// Send a packet to a specific address.
    pub async fn send_to(&self, addr: SocketAddr, packet: &ProtocolPacket) -> std::io::Result<usize> {
        let encoded = packet.encode();
        self.socket.send_to(&encoded, addr).await
    }

    /// Send a packet on an existing connection.
    pub async fn send_on_connection(
        &self,
        addr: SocketAddr,
        packet: &ProtocolPacket,
    ) -> std::io::Result<usize> {
        self.send_to(addr, packet).await
    }

    /// Remove timed-out connections and return their addresses.
    pub fn remove_timed_out(&mut self) -> Vec<(SocketAddr, u64)> {
        let mut removed = Vec::new();
        self.connections.retain(|addr, conn| {
            if conn.is_timed_out() {
                removed.push((*addr, conn.client_id));
                false
            } else {
                true
            }
        });
        removed
    }
}

/// Spawn a background task that sends heartbeats to all connected peers.
pub fn spawn_heartbeat_task(
    manager: Arc<tokio::sync::Mutex<ConnectionManager>>,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            let mut mgr = manager.lock().await;
            let addrs: Vec<SocketAddr> = mgr.connections.keys().copied().collect();
            for addr in addrs {
                if let Some(conn) = mgr.connections.get(&addr) {
                    let packet = conn.heartbeat_packet();
                    let _ = mgr.send_to(addr, &packet).await;
                }
            }
        }
    });
}
