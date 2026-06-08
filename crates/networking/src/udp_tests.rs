//! Tests for async UDP socket abstraction.

use std::net::SocketAddr;
use crate::udp::*;

#[tokio::test]
async fn async_udp_socket_bind() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let socket = AsyncUdpSocket::bind(addr).await.unwrap();
    let local = socket.local_addr().unwrap();
    assert!(local.port() > 0);
}

#[tokio::test]
async fn async_udp_socket_send_recv_roundtrip() {
    let addr1: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let socket1 = AsyncUdpSocket::bind(addr1).await.unwrap();
    let socket2 = AsyncUdpSocket::bind(addr2).await.unwrap();

    let local2 = socket2.local_addr().unwrap();
    let msg = b"hello udp";
    let sent = socket1.send_to(msg, local2).await.unwrap();
    assert_eq!(sent, msg.len());

    let mut buf = [0u8; 1024];
    let (len, sender) = socket2.recv_from(&mut buf).await.unwrap();
    assert_eq!(len, msg.len());
    assert_eq!(&buf[..len], msg);
    assert_eq!(sender, socket1.local_addr().unwrap());
}

#[tokio::test]
async fn async_udp_socket_arc_clone() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let socket = AsyncUdpSocket::bind(addr).await.unwrap();
    let clone = socket.arc_clone();
    assert_eq!(clone.local_addr().unwrap(), socket.local_addr().unwrap());
}

#[tokio::test]
async fn udp_pipeline_creation() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (socket, _tx, _rx) = create_udp_pipeline(addr, 1024).await.unwrap();
    let local = socket.local_addr().unwrap();
    assert!(local.port() > 0);
}

#[tokio::test]
async fn udp_receiver_and_sender() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let socket = AsyncUdpSocket::bind(addr).await.unwrap();
    let arc = socket.arc_clone();
    let local = socket.local_addr().unwrap();

    let mut rx = spawn_udp_receiver(arc.clone(), 1024);
    let tx = spawn_udp_sender(arc);

    let msg = b"through pipeline";
    tx.send((local, msg.to_vec())).unwrap();

    // Give the sender a moment to transmit.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let received = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        rx.recv(),
    )
    .await
    .unwrap();

    let (from, data) = received.unwrap();
    assert_eq!(data, msg);
    assert_eq!(from, local);
}
