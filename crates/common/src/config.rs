//! Application configuration types.

use crate::protocol::Resolution;
use serde::{Deserialize, Serialize};

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Display name for this machine on the network.
    pub hostname: String,
    /// Signaling port for TCP connections.
    pub signaling_port: u16,
    /// Default streaming resolution.
    pub resolution: Resolution,
    /// Default framerate.
    pub framerate: u32,
    /// Video bitrate in kbps.
    pub video_bitrate_kbps: u32,
    /// Whether to attempt hardware encoding.
    pub prefer_hw_encoding: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hostname: gethostname(),
            signaling_port: crate::protocol::DEFAULT_SIGNALING_PORT,
            resolution: Resolution::fhd(),
            framerate: 60,
            video_bitrate_kbps: 15_000,
            prefer_hw_encoding: true,
        }
    }
}

/// Detected session type on this Linux system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    X11,
    Wayland,
    Unknown,
}

impl SessionType {
    /// Detect the current session type from environment variables.
    pub fn detect() -> Self {
        match std::env::var("XDG_SESSION_TYPE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "x11" => SessionType::X11,
            "wayland" => SessionType::Wayland,
            _ => SessionType::Unknown,
        }
    }
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionType::X11 => write!(f, "X11"),
            SessionType::Wayland => write!(f, "Wayland"),
            SessionType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Get the system hostname, falling back to "unknown".
fn gethostname() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}
