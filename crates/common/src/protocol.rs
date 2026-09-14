//! Protocol message definitions for signaling between host and client.
//!
//! Messages are serialized as JSON over a TCP signaling channel.
//! Input events use a compact binary format over WebRTC DataChannel.

use serde::{Deserialize, Serialize};

/// Top-level signaling message envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SignalingMessage {
    /// Host announces its presence and capabilities.
    Announce(AnnouncePayload),
    /// Client sends PIN to authenticate.
    PinRequest(PinRequestPayload),
    /// Host responds to PIN attempt.
    PinResponse(PinResponsePayload),
    /// WebRTC SDP offer (from host).
    SdpOffer(SdpPayload),
    /// WebRTC SDP answer (from client).
    SdpAnswer(SdpPayload),
    /// WebRTC ICE candidate exchange.
    IceCandidate(IceCandidatePayload),
    /// Session control messages.
    SessionControl(SessionControlPayload),
    /// Input event forwarded from client to host.
    Input(InputEvent),
}

/// Host capabilities announced during discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncePayload {
    /// Human-readable hostname.
    pub hostname: String,
    /// Available display resolution on the host.
    pub resolution: Resolution,
    /// Whether the host supports hardware encoding.
    pub hw_encoding: bool,
    /// Protocol version for compatibility checks.
    pub protocol_version: u32,
}

/// PIN authentication request from client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinRequestPayload {
    /// The 4-digit PIN entered by the user.
    pub pin: String,
    /// Client's hostname for display purposes.
    pub client_hostname: String,
}

/// PIN authentication response from host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinResponsePayload {
    /// Whether the PIN was accepted.
    pub accepted: bool,
    /// Optional rejection reason.
    pub reason: Option<String>,
}

/// WebRTC Session Description Protocol payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdpPayload {
    /// SDP type: "offer" or "answer".
    pub sdp_type: String,
    /// The SDP string.
    pub sdp: String,
}

/// WebRTC ICE candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidatePayload {
    /// The ICE candidate string.
    pub candidate: String,
    /// SDP media line index.
    pub sdp_m_line_index: u32,
}

/// Session lifecycle control.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum SessionControlPayload {
    /// Request to start streaming.
    Start {
        /// Desired resolution.
        resolution: Resolution,
        /// Desired framerate.
        framerate: u32,
    },
    /// Request to stop streaming.
    Stop,
    /// Graceful disconnect.
    Disconnect,
    /// Ping for keepalive.
    Ping { timestamp_ms: u64 },
    /// Pong response.
    Pong { timestamp_ms: u64 },
}

/// Display resolution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Standard 1080p resolution.
    pub fn fhd() -> Self {
        Self::new(1920, 1080)
    }

    /// Standard 720p resolution.
    pub fn hd() -> Self {
        Self::new(1280, 720)
    }
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Input event sent from client to host over WebRTC DataChannel.
///
/// These are serialized as JSON for simplicity in the MVP.
/// A binary format (bincode/flatbuffers) can be used for lower latency later.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum InputEvent {
    /// Mouse movement (relative coordinates).
    MouseMove { dx: f64, dy: f64 },
    /// Mouse movement (absolute coordinates within the virtual display).
    MouseMoveAbsolute { x: f64, y: f64 },
    /// Mouse button press/release.
    MouseButton {
        button: MouseButton,
        pressed: bool,
    },
    /// Mouse scroll wheel.
    MouseScroll { dx: f64, dy: f64 },
    /// Keyboard key press/release.
    Key {
        /// Linux evdev keycode.
        keycode: u32,
        pressed: bool,
    },
}

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

/// Current protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

/// Default signaling port.
pub const DEFAULT_SIGNALING_PORT: u16 = 9876;

/// mDNS service type for discovery.
pub const MDNS_SERVICE_TYPE: &str = "_screenextend._tcp.local.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signaling_message_serialization() {
        let msg = SignalingMessage::Announce(AnnouncePayload {
            hostname: "my-desktop".into(),
            resolution: Resolution::fhd(),
            hw_encoding: false,
            protocol_version: PROTOCOL_VERSION,
        });
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: SignalingMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            SignalingMessage::Announce(p) => {
                assert_eq!(p.hostname, "my-desktop");
                assert_eq!(p.resolution, Resolution::fhd());
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_input_event_serialization() {
        let evt = InputEvent::MouseMove { dx: 10.5, dy: -3.2 };
        let json = serde_json::to_string(&evt).unwrap();
        let decoded: InputEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            InputEvent::MouseMove { dx, dy } => {
                assert!((dx - 10.5).abs() < f64::EPSILON);
                assert!((dy - (-3.2)).abs() < f64::EPSILON);
            }
            _ => panic!("Wrong variant"),
        }
    }
}
