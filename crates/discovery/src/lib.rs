//! # ScreenExtend Discovery
//!
//! mDNS service discovery and PIN-based handshake for peer-to-peer
//! connection establishment on the local network.

pub mod handshake;
pub mod mdns;

pub use handshake::{HandshakeClient, HandshakeServer, SignalingChannel};
pub use mdns::{get_local_ips, DiscoveryService};
