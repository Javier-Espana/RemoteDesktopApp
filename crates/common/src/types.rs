//! Shared types used across modules.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Information about a discovered peer on the network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer's human-readable hostname.
    pub hostname: String,
    /// Peer's network address (IP + signaling port).
    pub addr: SocketAddr,
    /// Whether this peer is acting as a host (offering screen extension).
    pub is_host: bool,
}

/// Represents the current state of a streaming session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Not connected to any peer.
    Disconnected,
    /// Discovering peers on the network.
    Discovering,
    /// Found a peer, waiting for PIN authentication.
    Authenticating,
    /// Authenticated, negotiating WebRTC connection.
    Negotiating,
    /// Actively streaming.
    Streaming,
    /// An error occurred.
    Error,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionState::Disconnected => write!(f, "Disconnected"),
            SessionState::Discovering => write!(f, "Discovering..."),
            SessionState::Authenticating => write!(f, "Authenticating..."),
            SessionState::Negotiating => write!(f, "Negotiating..."),
            SessionState::Streaming => write!(f, "Streaming"),
            SessionState::Error => write!(f, "Error"),
        }
    }
}

/// Application role — each instance acts as either Host or Client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppRole {
    /// Host: shares its extended display to the network.
    Host,
    /// Client: receives and renders the extended display.
    Client,
}

impl std::fmt::Display for AppRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppRole::Host => write!(f, "Host"),
            AppRole::Client => write!(f, "Client"),
        }
    }
}
