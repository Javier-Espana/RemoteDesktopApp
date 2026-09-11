//! Client-side input event capture.
//!
//! Translates window / widget input signals into generic `InputEvent` messages.

use screenextend_common::protocol::{InputEvent, MouseButton};
use tokio::sync::mpsc;
use tracing::warn;

/// Captures user input on the client side and sends it to the output channel.
#[derive(Clone)]
pub struct InputCapture {
    sender: mpsc::Sender<InputEvent>,
}

impl InputCapture {
    /// Create a new InputCapture instance with a target event channel.
    pub fn new(sender: mpsc::Sender<InputEvent>) -> Self {
        Self { sender }
    }

    /// Dispatch relative mouse movement.
    pub fn handle_mouse_motion(&self, dx: f64, dy: f64) {
        let _ = self.sender.try_send(InputEvent::MouseMove { dx, dy });
    }

    /// Dispatch absolute mouse movement (e.g. cursor coordinates within the video viewport).
    pub fn handle_mouse_absolute(&self, x: f64, y: f64) {
        let _ = self.sender.try_send(InputEvent::MouseMoveAbsolute { x, y });
    }

    /// Dispatch mouse button press or release.
    /// GTK button numbers: 1 = Left, 2 = Middle, 3 = Right, 8 = Back, 9 = Forward
    pub fn handle_mouse_button(&self, button_code: u32, pressed: bool) {
        let button = match button_code {
            1 => MouseButton::Left,
            2 => MouseButton::Middle,
            3 => MouseButton::Right,
            8 => MouseButton::Back,
            9 => MouseButton::Forward,
            other => {
                warn!("Unmapped mouse button code: {}", other);
                return;
            }
        };

        let _ = self.sender.try_send(InputEvent::MouseButton {
            button,
            pressed,
        });
    }

    /// Dispatch scroll wheel action.
    pub fn handle_scroll(&self, dx: f64, dy: f64) {
        let _ = self.sender.try_send(InputEvent::MouseScroll { dx, dy });
    }

    /// Dispatch keyboard key event (using Linux evdev / XKB scancode).
    pub fn handle_key(&self, keycode: u32, pressed: bool) {
        let _ = self.sender.try_send(InputEvent::Key { keycode, pressed });
    }
}
