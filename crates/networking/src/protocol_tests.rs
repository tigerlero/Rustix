//! Tests for networking protocol, serialization, and transport.

use std::net::SocketAddr;
use crate::*;

// ---------- serialize.rs ----------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TestMessage {
    id: u64,
    name: String,
    values: Vec<f32>,
}

#[test]
fn serialize_roundtrip() {
    let msg = TestMessage { id: 42, name: "hello".to_string(), values: vec![1.0, 2.0, 3.0] };
    let bytes = serialize::serialize(&msg).unwrap();
    let decoded: TestMessage = serialize::deserialize(&bytes).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn serialize_unchecked_roundtrip() {
    let msg = TestMessage { id: 7, name: "test".to_string(), values: vec![] };
    let bytes = serialize::serialize_unchecked(&msg);
    let decoded: TestMessage = serialize::deserialize_unchecked(&bytes);
    assert_eq!(msg, decoded);
}

// ---------- protocol.rs ----------

#[test]
fn packet_type_to_u8_roundtrip() {
    assert_eq!(PacketType::HandshakeRequest.to_u8(), 0);
    assert_eq!(PacketType::HandshakeResponse.to_u8(), 1);
    assert_eq!(PacketType::Heartbeat.to_u8(), 2);
    assert_eq!(PacketType::Reliable.to_u8(), 3);
    assert_eq!(PacketType::Unreliable.to_u8(), 4);
    assert_eq!(PacketType::Disconnect.to_u8(), 5);
}

#[test]
fn packet_type_from_u8() {
    assert_eq!(PacketType::from_u8(0), Some(PacketType::HandshakeRequest));
    assert_eq!(PacketType::from_u8(3), Some(PacketType::Reliable));
    assert_eq!(PacketType::from_u8(5), Some(PacketType::Disconnect));
    assert_eq!(PacketType::from_u8(99), None);
}

#[test]
fn protocol_packet_encode_decode() {
    let packet = ProtocolPacket {
        packet_type: PacketType::Reliable,
        sequence: 123,
        ack: 456,
        payload: vec![1, 2, 3, 4],
    };
    let encoded = packet.encode();
    assert_eq!(encoded.len(), 9); // 1 type + 2 seq + 2 ack + 4 payload

    let decoded = ProtocolPacket::decode(&encoded).unwrap();
    assert_eq!(decoded.packet_type, PacketType::Reliable);
    assert_eq!(decoded.sequence, 123);
    assert_eq!(decoded.ack, 456);
    assert_eq!(decoded.payload, vec![1, 2, 3, 4]);
}

#[test]
fn protocol_packet_decode_too_short() {
    assert!(ProtocolPacket::decode(&[0, 0]).is_none());
    assert!(ProtocolPacket::decode(&[]).is_none());
}

#[test]
fn protocol_packet_decode_unknown_type() {
    assert!(ProtocolPacket::decode(&[99, 0, 0, 0, 0]).is_none());
}

#[test]
fn virtual_connection_new_handshaking() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let conn = VirtualConnection::new(addr, 1);
    assert_eq!(conn.state, ConnectionState::Handshaking);
    assert_eq!(conn.client_id, 1);
    assert_eq!(conn.next_sequence, 1);
    assert_eq!(conn.last_received_seq, 0);
    assert!(conn.pending_ack.is_empty());
    assert!(conn.reliable_inbox.is_empty());
    assert!(conn.unreliable_inbox.is_empty());
}

#[test]
fn virtual_connection_send_reliable() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    let packet = conn.send_reliable(vec![1, 2, 3]);
    assert_eq!(packet.packet_type, PacketType::Reliable);
    assert_eq!(packet.sequence, 1);
    assert_eq!(conn.next_sequence, 2);
    assert_eq!(conn.pending_ack.len(), 1);
}

#[test]
fn virtual_connection_send_unreliable() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    conn.last_received_seq = 5;
    let packet = conn.send_unreliable(vec![9, 8, 7]);
    assert_eq!(packet.packet_type, PacketType::Unreliable);
    assert_eq!(packet.sequence, 0);
    assert_eq!(packet.ack, 5);
}

#[test]
fn virtual_connection_receive_reliable() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    let packet = ProtocolPacket {
        packet_type: PacketType::Reliable,
        sequence: 10,
        ack: 0,
        payload: vec![5, 6],
    };
    conn.receive_packet(packet);
    assert_eq!(conn.last_received_seq, 10);
    assert_eq!(conn.reliable_inbox.len(), 1);
    assert_eq!(conn.reliable_inbox[0], vec![5, 6]);
}

#[test]
fn virtual_connection_receive_duplicate_ignored() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    let packet = ProtocolPacket {
        packet_type: PacketType::Reliable,
        sequence: 10,
        ack: 0,
        payload: vec![1],
    };
    conn.receive_packet(packet.clone());
    conn.receive_packet(packet);
    assert_eq!(conn.reliable_inbox.len(), 1);
}

#[test]
fn virtual_connection_receive_unreliable() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    let packet = ProtocolPacket {
        packet_type: PacketType::Unreliable,
        sequence: 0,
        ack: 0,
        payload: vec![7, 8],
    };
    conn.receive_packet(packet);
    assert_eq!(conn.unreliable_inbox.len(), 1);
}

#[test]
fn virtual_connection_handshake_request() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    let packet = ProtocolPacket {
        packet_type: PacketType::HandshakeRequest,
        sequence: 0,
        ack: 0,
        payload: vec![],
    };
    conn.receive_packet(packet);
    assert_eq!(conn.state, ConnectionState::Connected);
}

#[test]
fn virtual_connection_handshake_response() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    let packet = ProtocolPacket {
        packet_type: PacketType::HandshakeResponse,
        sequence: 0,
        ack: 0,
        payload: vec![],
    };
    conn.receive_packet(packet);
    assert_eq!(conn.state, ConnectionState::Connected);
}

#[test]
fn virtual_connection_disconnect() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    conn.state = ConnectionState::Connected;
    let packet = ProtocolPacket {
        packet_type: PacketType::Disconnect,
        sequence: 0,
        ack: 0,
        payload: vec![],
    };
    conn.receive_packet(packet);
    assert_eq!(conn.state, ConnectionState::Disconnected);
}

#[test]
fn virtual_connection_heartbeat_updates_last_heartbeat() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    let before = conn.last_heartbeat;
    let packet = ProtocolPacket {
        packet_type: PacketType::Heartbeat,
        sequence: 0,
        ack: 0,
        payload: vec![],
    };
    conn.receive_packet(packet);
    assert!(conn.last_heartbeat > before);
}

#[test]
fn virtual_connection_is_timed_out() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    conn.last_heartbeat = tokio::time::Instant::now() - std::time::Duration::from_secs(10);
    conn.disconnect_timeout = std::time::Duration::from_secs(5);
    assert!(conn.is_timed_out());
}

#[test]
fn virtual_connection_not_timed_out() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let conn = VirtualConnection::new(addr, 1);
    assert!(!conn.is_timed_out());
}

#[test]
fn virtual_connection_ack_removes_pending() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    conn.send_reliable(vec![1, 2, 3]);
    assert_eq!(conn.pending_ack.len(), 1);
    conn.ack(1);
    assert!(conn.pending_ack.is_empty());
}

#[test]
fn virtual_connection_heartbeat_packet() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let mut conn = VirtualConnection::new(addr, 1);
    conn.last_received_seq = 42;
    let packet = conn.heartbeat_packet();
    assert_eq!(packet.packet_type, PacketType::Heartbeat);
    assert_eq!(packet.ack, 42);
    assert!(packet.payload.is_empty());
}

#[test]
fn virtual_connection_pending_retransmits_empty() {
    let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let conn = VirtualConnection::new(addr, 1);
    let retrans = conn.pending_retransmits(std::time::Duration::from_secs(1));
    assert!(retrans.is_empty());
}

// ---------- lib.rs types ----------

#[test]
fn client_id_default() {
    let id = ClientId::default();
    assert_eq!(id.0, 0);
}

#[test]
fn in_memory_transport_new() {
    let id = ClientId(1);
    let transport = InMemoryTransport::<u32>::new(id);
    assert_eq!(transport.local_id, id);
    assert!(!transport.connected);
    assert!(transport.outgoing.is_empty());
    assert!(transport.incoming.is_empty());
}

#[test]
fn in_memory_transport_connect_disconnect() {
    let mut transport = InMemoryTransport::<u32>::new(ClientId(1));
    transport.connect();
    assert!(transport.connected);
    assert!(matches!(transport.receive(), Some(NetworkEvent::Connected(_))));

    transport.disconnect();
    assert!(!transport.connected);
    assert!(matches!(transport.receive(), Some(NetworkEvent::Disconnected(_))));
}

#[test]
fn in_memory_transport_send() {
    let mut transport = InMemoryTransport::<u32>::new(ClientId(1));
    transport.send(ClientId(2), 42);
    assert_eq!(transport.outgoing.len(), 1);
    assert_eq!(transport.outgoing[0], (ClientId(2), 42));
}

#[test]
fn in_memory_transport_broadcast() {
    let mut transport = InMemoryTransport::<u32>::new(ClientId(1));
    transport.broadcast(99, &[ClientId(2), ClientId(3), ClientId(1)]);
    assert_eq!(transport.outgoing.len(), 2);
    assert!(!transport.outgoing.iter().any(|(id, _)| *id == ClientId(1)));
}

#[test]
fn in_memory_transport_receive_fifo() {
    let mut transport = InMemoryTransport::<u32>::new(ClientId(1));
    transport.push_incoming(NetworkEvent::Connected(ClientId(1)));
    transport.push_incoming(NetworkEvent::Message(Message { from: ClientId(2), payload: 42 }));

    let first = transport.receive().unwrap();
    assert!(matches!(first, NetworkEvent::Connected(_)));

    let second = transport.receive().unwrap();
    assert!(matches!(second, NetworkEvent::Message(_)));

    assert!(transport.receive().is_none());
}
