//! Tests for NAT punch-through, relay, and rendezvous.

use std::net::SocketAddr;
use crate::nat::*;

// ---------- RendezvousMessage ----------

#[test]
fn rendezvous_message_roundtrip() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let msgs = vec![
        RendezvousMessage::Register { session_id: "sess".into() },
        RendezvousMessage::YourEndpoint { addr },
        RendezvousMessage::PeerEndpoint { addr },
        RendezvousMessage::KeepAlive,
    ];
    for msg in &msgs {
        let bytes = crate::serialize(msg).unwrap();
        let decoded: RendezvousMessage = crate::deserialize(&bytes).unwrap();
        assert_eq!(format!("{:?}", msg), format!("{:?}", decoded));
    }
}

// ---------- RelayPacket ----------

#[test]
fn relay_packet_roundtrip() {
    let packet = RelayPacket {
        sender_client_id: 1,
        target_client_id: 2,
        payload: vec![1, 2, 3],
    };
    let bytes = crate::serialize(&packet).unwrap();
    let decoded: RelayPacket = crate::deserialize(&bytes).unwrap();
    assert_eq!(decoded.sender_client_id, 1);
    assert_eq!(decoded.target_client_id, 2);
    assert_eq!(decoded.payload, vec![1, 2, 3]);
}

// ---------- ConnectionMode ----------

#[test]
fn connection_mode_debug() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mode = ConnectionMode::Direct(addr);
    let s = format!("{:?}", mode);
    assert!(s.contains("Direct"));
}

// ---------- RelayServer ----------

#[tokio::test]
async fn relay_server_new_and_register() {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server = RelayServer::new(socket);
    let addr: SocketAddr = "127.0.0.1:5678".parse().unwrap();
    server.register_client(1, addr);
    // registration is internal state, tested by not panicking
}

#[tokio::test]
async fn relay_server_unregister() {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server = RelayServer::new(socket);
    let addr: SocketAddr = "127.0.0.1:5678".parse().unwrap();
    server.register_client(1, addr);
    server.unregister_client(1);
    // unregistration is internal state, tested by not panicking
}

// ---------- RelayClient ----------

#[tokio::test]
async fn relay_client_new_and_getters() {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();
    let client = RelayClient::new(socket, addr, 42);
    assert_eq!(client.local_client_id(), 42);
    assert_eq!(client.relay_addr(), addr);
}

// ---------- NatPunchThrough ----------

#[tokio::test]
async fn nat_punch_through_new() {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = "127.0.0.1:1111".parse().unwrap();
    let punch = NatPunchThrough::new(socket, addr);
    let s = format!("{:?}", punch);
    assert!(s.contains("NatPunchThrough"));
}

// ---------- RendezvousClient ----------

#[tokio::test]
async fn rendezvous_client_new() {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = "127.0.0.1:2222".parse().unwrap();
    let client = RendezvousClient::new(socket, addr, "session".into());
    let s = format!("{:?}", client);
    assert!(s.contains("RendezvousClient"));
    assert!(s.contains("session"));
}
