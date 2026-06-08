//! Async UDP socket abstraction using `tokio`.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

/// A thin async wrapper around `tokio::net::UdpSocket` for sending and receiving raw bytes.
#[derive(Debug)]
pub struct AsyncUdpSocket {
    socket: Arc<UdpSocket>,
}

impl AsyncUdpSocket {
    /// Bind the UDP socket to a local address.
    pub async fn bind(addr: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        Ok(Self {
            socket: Arc::new(socket),
        })
    }

    /// Send `data` to `target` without establishing a connection.
    pub async fn send_to(&self, data: &[u8], target: SocketAddr) -> std::io::Result<usize> {
        self.socket.send_to(data, target).await
    }

    /// Receive a single datagram, returning `(bytes_read, sender_addr, buffer)`.
    ///
    /// The caller should slice `buffer` to `bytes_read` to read the actual payload.
    pub async fn recv_from(&self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf).await
    }

    /// Returns the local address this socket is bound to.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Clone the inner `Arc<UdpSocket>` for use in spawned tasks.
    pub fn arc_clone(&self) -> Arc<UdpSocket> {
        Arc::clone(&self.socket)
    }
}

/// A UDP receiver task that continuously reads incoming datagrams and forwards
/// them to a channel.
///
/// Spawn this with `tokio::spawn` and consume `(SocketAddr, Vec<u8>)` pairs
/// from the returned `Receiver`.
pub fn spawn_udp_receiver(
    socket: Arc<UdpSocket>,
    buffer_size: usize,
) -> mpsc::UnboundedReceiver<(SocketAddr, Vec<u8>)> {
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut buf = vec![0u8; buffer_size];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let payload = buf[..len].to_vec();
                    if tx.send((addr, payload)).is_err() {
                        // Receiver dropped — shut down.
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("UDP recv error: {}", e);
                    break;
                }
            }
        }
    });

    rx
}

/// A UDP sender task that accepts `(SocketAddr, Vec<u8>)` pairs from a channel
/// and sends them out over the socket.
pub fn spawn_udp_sender(
    socket: Arc<UdpSocket>,
) -> mpsc::UnboundedSender<(SocketAddr, Vec<u8>)> {
    let (tx, mut rx) = mpsc::unbounded_channel::<(SocketAddr, Vec<u8>)>();

    tokio::spawn(async move {
        while let Some((addr, data)) = rx.recv().await {
            if let Err(e) = socket.send_to(&data, addr).await {
                tracing::error!("UDP send error to {}: {}", addr, e);
            }
        }
    });

    tx
}

/// Convenience: create a bound UDP socket with a paired sender/receiver channel pipeline.
///
/// Returns `(AsyncUdpSocket, sender, receiver)`.
pub async fn create_udp_pipeline(
    bind_addr: SocketAddr,
    recv_buffer_size: usize,
) -> std::io::Result<(AsyncUdpSocket, mpsc::UnboundedSender<(SocketAddr, Vec<u8>)>, mpsc::UnboundedReceiver<(SocketAddr, Vec<u8>)>)> {
    let socket = AsyncUdpSocket::bind(bind_addr).await?;
    let arc = socket.arc_clone();
    let rx = spawn_udp_receiver(arc.clone(), recv_buffer_size);
    let tx = spawn_udp_sender(arc);
    Ok((socket, tx, rx))
}
