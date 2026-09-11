//! Input event serialization for the DataChannel / network transport.

pub use screenextend_common::protocol::{InputEvent, MouseButton};
use anyhow::{Context, Result};

/// Serialize an `InputEvent` into byte payload (JSON for MVP, compact & easily debuggable).
pub fn serialize_event(event: &InputEvent) -> Result<Vec<u8>> {
    serde_json::to_vec(event).context("Failed to serialize InputEvent")
}

/// Deserialize an `InputEvent` from byte payload.
pub fn deserialize_event(data: &[u8]) -> Result<InputEvent> {
    serde_json::from_slice(data).context("Failed to deserialize InputEvent")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_roundtrip() {
        let ev = InputEvent::MouseButton {
            button: MouseButton::Left,
            pressed: true,
        };
        let bytes = serialize_event(&ev).unwrap();
        let decoded = deserialize_event(&bytes).unwrap();
        match decoded {
            InputEvent::MouseButton { button, pressed } => {
                assert_eq!(button, MouseButton::Left);
                assert!(pressed);
            }
            _ => panic!("Unexpected event"),
        }
    }
}
