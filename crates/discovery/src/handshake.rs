//! PIN-based handshake and signaling channel over TCP.
//!
//! The host generates a 4-digit PIN displayed in the UI.
//! The client enters the PIN to authenticate and establish
//! a persistent TCP connection used for WebRTC signaling.

use anyhow::{bail, Context, Result};
use screenextend_common::protocol::*;
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

/// A bidirectional signaling channel over TCP using newline-delimited JSON.
pub struct SignalingChannel {
    reader: BufReader<tokio::io::ReadHalf<TcpStream>>,
    writer: BufWriter<tokio::io::WriteHalf<TcpStream>>,
    peer_addr: SocketAddr,
}

impl SignalingChannel {
    /// Create a new signaling channel from a connected TcpStream.
    fn new(stream: TcpStream, peer_addr: SocketAddr) -> Self {
        let (read_half, write_half) = tokio::io::split(stream);
        Self {
            reader: BufReader::new(read_half),
            writer: BufWriter::new(write_half),
            peer_addr,
        }
    }

    /// Send a signaling message.
    pub async fn send(&mut self, msg: &SignalingMessage) -> Result<()> {
        let mut json = serde_json::to_string(msg)
            .context("Failed to serialize signaling message")?;
        json.push('\n');
        self.writer
            .write_all(json.as_bytes())
            .await
            .context("Failed to write to signaling channel")?;
        self.writer
            .flush()
            .await
            .context("Failed to flush signaling channel")?;
        Ok(())
    }

    /// Receive the next signaling message.
    pub async fn recv(&mut self) -> Result<SignalingMessage> {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .await
            .context("Failed to read from signaling channel")?;

        if bytes_read == 0 {
            bail!("Signaling channel closed by peer");
        }

        serde_json::from_str(line.trim())
            .context("Failed to deserialize signaling message")
    }

    /// Get the peer's address.
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }
}

/// Server-side handshake handler (runs on the host).
pub struct HandshakeServer {
    listener: TcpListener,
    pin: String,
}

impl HandshakeServer {
    /// Create a new handshake server bound to the given port.
    pub async fn new(port: u16) -> Result<Self> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
            .await
            .context(format!("Failed to bind signaling server on port {}", port))?;

        // Generate random 4-digit PIN
        let pin = format!("{:04}", rand::random_range(0..10000u32));

        info!("Handshake server listening on port {}", port);
        info!("PIN: {}", pin);

        Ok(Self { listener, pin })
    }

    /// Get the generated PIN (to display in the UI).
    pub fn pin(&self) -> &str {
        &self.pin
    }

    /// Get the bound local address of the listener.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr().context("Failed to get local address")
    }

    /// Wait for a client to connect and authenticate.
    ///
    /// Returns a `SignalingChannel` on successful authentication.
    pub async fn accept(&self) -> Result<SignalingChannel> {
        loop {
            let (stream, peer_addr) = self
                .listener
                .accept()
                .await
                .context("Failed to accept connection")?;

            info!("Incoming connection from {}", peer_addr);

            let mut channel = SignalingChannel::new(stream, peer_addr);

            // Read PIN request
            match channel.recv().await {
                Ok(SignalingMessage::PinRequest(req)) => {
                    info!("PIN attempt from {} ({})", req.client_hostname, peer_addr);

                    if req.pin == self.pin {
                        info!("PIN accepted for {}", req.client_hostname);
                        channel
                            .send(&SignalingMessage::PinResponse(PinResponsePayload {
                                accepted: true,
                                reason: None,
                            }))
                            .await?;
                        return Ok(channel);
                    } else {
                        warn!("Wrong PIN from {}", req.client_hostname);
                        channel
                            .send(&SignalingMessage::PinResponse(PinResponsePayload {
                                accepted: false,
                                reason: Some("Invalid PIN".into()),
                            }))
                            .await?;
                        // Continue waiting for another connection
                    }
                }
                Ok(other) => {
                    warn!("Unexpected message from {}: {:?}", peer_addr, other);
                }
                Err(e) => {
                    error!("Error reading from {}: {}", peer_addr, e);
                }
            }
        }
    }
}

/// Client-side handshake handler.
pub struct HandshakeClient;

impl HandshakeClient {
    /// Connect to a host and authenticate with the given PIN.
    ///
    /// Returns a `SignalingChannel` on successful authentication.
    pub async fn connect(addr: SocketAddr, pin: &str) -> Result<SignalingChannel> {
        info!("Connecting to host at {}", addr);

        let stream = TcpStream::connect(addr)
            .await
            .context(format!("Failed to connect to host at {}", addr))?;

        let mut channel = SignalingChannel::new(stream, addr);

        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".into());

        // Send PIN
        channel
            .send(&SignalingMessage::PinRequest(PinRequestPayload {
                pin: pin.to_string(),
                client_hostname: hostname,
            }))
            .await?;

        // Wait for response
        match channel.recv().await? {
            SignalingMessage::PinResponse(resp) => {
                if resp.accepted {
                    info!("Authentication successful!");
                    Ok(channel)
                } else {
                    bail!(
                        "Authentication failed: {}",
                        resp.reason.unwrap_or_else(|| "Unknown reason".into())
                    );
                }
            }
            other => {
                bail!("Unexpected response: {:?}", other);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handshake_pin_success() {
        let server = HandshakeServer::new(0).await.unwrap();
        let pin = server.pin().to_string();
        let addr = server.local_addr().unwrap();

        let server_handle = tokio::spawn(async move {
            let mut channel = server.accept().await.unwrap();
            channel.send(&SignalingMessage::SessionControl(
                screenextend_common::protocol::SessionControlPayload::Ping { timestamp_ms: 42 },
            )).await.unwrap();
        });

        let mut client_channel = HandshakeClient::connect(addr, &pin).await.unwrap();
        match client_channel.recv().await.unwrap() {
            SignalingMessage::SessionControl(
                screenextend_common::protocol::SessionControlPayload::Ping { timestamp_ms }
            ) => {
                assert_eq!(timestamp_ms, 42);
            }
            _ => panic!("Expected Ping message"),
        }

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handshake_pin_failure() {
        let server = HandshakeServer::new(0).await.unwrap();
        let addr = server.local_addr().unwrap();

        let server_handle = tokio::spawn(async move {
            // Server will reject wrong PIN and continue waiting
            let _ = server.accept().await;
        });

        let result = HandshakeClient::connect(addr, "99999").await;
        assert!(result.is_err(), "Expected handshake failure on wrong PIN");
        server_handle.abort();
    }
}
