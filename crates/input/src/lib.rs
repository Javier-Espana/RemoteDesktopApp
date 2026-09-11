//! # ScreenExtend Input
//!
//! Input event capture on the client side and input event injection on the host side
//! via `/dev/uinput`.

pub mod capture;
pub mod injection;
pub mod protocol;

pub use capture::InputCapture;
pub use injection::InputInjector;
pub use protocol::{deserialize_event, serialize_event};
