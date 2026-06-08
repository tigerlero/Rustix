//! NAT punch-through and relay server support.
//!
//! Provides:
//! - `RendezvousClient` — exchanges public endpoints with a rendezvous server.
//! - `NatPunchThrough` — sends/receives UDP hole-punching packets.
//! - `RelayServer` / `RelayClient` — fallback when direct connection fails.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

/// Messages exchanged with the rendezvous server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RendezvousMessage {
    /// Client → Server: request registration for a session.
    Register { session_id: String },
    /// Server → Client: your public NAT-mapped endpoint.
    YourEndpoint { addr: SocketAddr },
    /// Server → Client: peer's public endpoint (after both register).
    PeerEndpoint { addr: SocketAddr },
    /// Client → Server: keep the registration alive.
    KeepAlive,
}

/// Client that talks to a rendezvous server to discover a peer's
/// public endpoint before attempting NAT punch-through.
pub struct RendezvousClient {
    socket: UdpSocket,
    server_addr: SocketAddr,
    session_id: String,
}

impl std::fmt::Debug for RendezvousClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RendezvousClient")
            .field("server_addr", &self.server_addr)
            .field("session_id", &self.session_id)
            .finish()
    }
}

impl RendezvousClient {
    pub fn new(socket: UdpSocket, server_addr: SocketAddr, session_id: String) -> Self {
        Self {
            socket,
            server_addr,
            session_id,
        }
    }

    /// Register with the rendezvous server and wait for the peer endpoint.
    pub async fn register_and_wait_peer(&self, timeout_ms: u64) -> Result<SocketAddr, String> {
        let register = RendezvousMessage::Register {
            session_id: self.session_id.clone(),
        };
        let bytes = crate::serialize(&register).map_err(|e| e.to_string())?;
        self.socket
            .send_to(&bytes, self.server_addr)
            .await
            .map_err(|e| e.to_string())?;

        let mut buf = vec![0u8; 1024];
        let dur = Duration::from_millis(timeout_ms);
        let (len, _) = timeout(dur, self.socket.recv_from(&mut buf))
            .await
            .map_err(|_| "Rendezvous timed out".to_string())?
            .map_err(|e| e.to_string())?;

        let msg: RendezvousMessage = crate::deserialize(&buf[..len]).map_err(|e| e.to_string())?;
        match msg {
            RendezvousMessage::PeerEndpoint { addr } => Ok(addr),
            other => Err(format!("Unexpected rendezvous response: {:?}", other)),
        }
    }

    /// Send a keep-alive so the rendezvous server doesn't drop the session.
    pub async fn keep_alive(&self) -> Result<(), String> {
        let msg = RendezvousMessage::KeepAlive;
        let bytes = crate::serialize(&msg).map_err(|e| e.to_string())?;
        self.socket
            .send_to(&bytes, self.server_addr)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// UDP hole-punching helper.
///
/// After both peers know each other's public endpoints (via rendezvous),
/// each sends a small burst of packets to the peer. The first packet
/// that gets through opens the NAT mapping for return traffic.
pub struct NatPunchThrough {
    socket: UdpSocket,
    peer_addr: SocketAddr,
}

impl std::fmt::Debug for NatPunchThrough {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatPunchThrough")
            .field("peer_addr", &self.peer_addr)
            .finish()
    }
}

impl NatPunchThrough {
    pub fn new(socket: UdpSocket, peer_addr: SocketAddr) -> Self {
        Self { socket, peer_addr }
    }

    /// Send a burst of hole-punch packets and wait for the peer response.
    ///
    /// Returns `Ok(())` as soon as a valid response from `peer_addr` is
    /// received. Returns `Err` on timeout.
    pub async fn punch(&self, burst_count: usize, timeout_ms: u64) -> Result<(), String> {
        let punch_data = b"PUNCH";
        for _ in 0..burst_count {
            let _ = self.socket.send_to(punch_data, self.peer_addr).await;
        }

        let mut buf = [0u8; 64];
        let dur = Duration::from_millis(timeout_ms);
        let (len, from) = timeout(dur, self.socket.recv_from(&mut buf))
            .await
            .map_err(|_| "NAT punch-through timed out".to_string())?
            .map_err(|e| e.to_string())?;

        if from == self.peer_addr && &buf[..len] == punch_data {
            Ok(())
        } else {
            Err("Unexpected packet during punch-through".to_string())
        }
    }
}

/// Packet wrapped by the relay protocol.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelayPacket {
    /// Client that originally sent this packet.
    pub sender_client_id: u64,
    /// Client that should receive this packet.
    pub target_client_id: u64,
    /// Opaque application payload.
    pub payload: Vec<u8>,
}

/// Minimal UDP relay server.
///
/// Clients send `RelayPacket`s to the relay; the relay looks up the
/// target's SocketAddr and forwards the payload. This is used as a
/// fallback when NAT punch-through fails.
pub struct RelayServer {
    socket: UdpSocket,
    /// client_id → SocketAddr
    clients: Arc<Mutex<HashMap<u64, SocketAddr>>>,
    /// SocketAddr → client_id (for sender identification)
    addr_to_client: Arc<Mutex<HashMap<SocketAddr, u64>>>,
}

impl std::fmt::Debug for RelayServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayServer")
            .field("clients", &self.clients.lock().unwrap().len())
            .finish()
    }
}

impl RelayServer {
    pub fn new(socket: UdpSocket) -> Self {
        Self {
            socket,
            clients: Arc::new(Mutex::new(HashMap::new())),
            addr_to_client: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a client so the relay knows where to forward packets.
    pub fn register_client(&self, client_id: u64, addr: SocketAddr) {
        self.clients.lock().unwrap().insert(client_id, addr);
        self.addr_to_client.lock().unwrap().insert(addr, client_id);
    }

    /// Remove a client from the relay.
    pub fn unregister_client(&self, client_id: u64) {
        if let Some(addr) = self.clients.lock().unwrap().remove(&client_id) {
            self.addr_to_client.lock().unwrap().remove(&addr);
        }
    }

    /// Run the relay forwarding loop. This never returns unless the
    /// socket errors.
    pub async fn run(&self) -> Result<(), String> {
        let mut buf = vec![0u8; 65535];
        loop {
            let (len, from) = self.socket.recv_from(&mut buf).await.map_err(|e| e.to_string())?;

            let packet: RelayPacket = match crate::deserialize(&buf[..len]) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let sender = {
                let map = self.addr_to_client.lock().unwrap();
                map.get(&from).copied().unwrap_or(0)
            };

            let target_addr = {
                let map = self.clients.lock().unwrap();
                map.get(&packet.target_client_id).copied()
            };

            if let Some(addr) = target_addr {
                let fwd = RelayPacket {
                    sender_client_id: sender,
                    target_client_id: packet.target_client_id,
                    payload: packet.payload,
                };
                let bytes = match crate::serialize(&fwd) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                let _ = self.socket.send_to(&bytes, addr).await;
            }
        }
    }
}

/// Client that sends/receives data via a relay server.
///
/// All outgoing packets are wrapped in `RelayPacket` and sent to the
/// relay address. Incoming packets are unwrapped and returned with
/// the original sender's `client_id`.
pub struct RelayClient {
    socket: UdpSocket,
    relay_addr: SocketAddr,
    local_client_id: u64,
}

impl std::fmt::Debug for RelayClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayClient")
            .field("relay_addr", &self.relay_addr)
            .field("local_client_id", &self.local_client_id)
            .finish()
    }
}

impl RelayClient {
    pub fn new(socket: UdpSocket, relay_addr: SocketAddr, local_client_id: u64) -> Self {
        Self {
            socket,
            relay_addr,
            local_client_id,
        }
    }

    /// Send a payload to `target_client_id` through the relay.
    pub async fn send_to(&self, target_client_id: u64, payload: &[u8]) -> Result<(), String> {
        let packet = RelayPacket {
            sender_client_id: self.local_client_id,
            target_client_id,
            payload: payload.to_vec(),
        };
        let bytes = crate::serialize(&packet).map_err(|e| e.to_string())?;
        self.socket
            .send_to(&bytes, self.relay_addr)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Receive a payload forwarded by the relay.
    ///
    /// Returns `(sender_client_id, payload)` on success.
    pub async fn recv(&self) -> Result<(u64, Vec<u8>), String> {
        let mut buf = vec![0u8; 65535];
        let (len, from) = self.socket.recv_from(&mut buf).await.map_err(|e| e.to_string())?;

        if from != self.relay_addr {
            return Err("Packet not from relay".to_string());
        }

        let packet: RelayPacket = crate::deserialize(&buf[..len]).map_err(|e| e.to_string())?;
        Ok((packet.sender_client_id, packet.payload))
    }

    pub fn local_client_id(&self) -> u64 {
        self.local_client_id
    }

    pub fn relay_addr(&self) -> SocketAddr {
        self.relay_addr
    }
}

/// High-level helper that attempts NAT punch-through and falls back to
/// the relay on failure.
pub async fn connect_with_fallback(
    socket: UdpSocket,
    rendezvous_addr: SocketAddr,
    session_id: String,
    relay_addr: SocketAddr,
    local_client_id: u64,
    punch_burst: usize,
    punch_timeout_ms: u64,
) -> Result<ConnectionMode, String> {
    let rendezvous = RendezvousClient::new(
        UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?,
        rendezvous_addr,
        session_id,
    );

    match rendezvous.register_and_wait_peer(5000).await {
        Ok(peer_addr) => {
            let punch = NatPunchThrough::new(socket, peer_addr);
            match punch.punch(punch_burst, punch_timeout_ms).await {
                Ok(()) => Ok(ConnectionMode::Direct(peer_addr)),
                Err(_) => {
                    let relay_socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;
                    let relay = RelayClient::new(relay_socket, relay_addr, local_client_id);
                    Ok(ConnectionMode::Relay(relay))
                }
            }
        }
        Err(_) => {
            let relay_socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;
            let relay = RelayClient::new(relay_socket, relay_addr, local_client_id);
            Ok(ConnectionMode::Relay(relay))
        }
    }
}

/// Result of a connection attempt with fallback.
#[derive(Debug)]
pub enum ConnectionMode {
    /// Direct UDP to the peer's public endpoint.
    Direct(SocketAddr),
    /// Fallback: packets forwarded through a relay.
    Relay(RelayClient),
}
