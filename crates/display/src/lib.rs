//! # ScreenExtend Display
//!
//! Virtual display management for creating and configuring
//! virtual monitors on the host machine.

pub mod x11;
pub mod wayland;

use screenextend_common::config::SessionType;
use anyhow::{bail, Result};

/// Trait for virtual display backends.
pub trait VirtualDisplay: Send {
    /// Create a virtual display with the given resolution.
    fn create(&mut self, width: u32, height: u32) -> Result<()>;
    /// Remove the virtual display.
    fn destroy(&mut self) -> Result<()>;
    /// Get the capture region (x, y, width, height) for the virtual display.
    fn capture_region(&self) -> (u32, u32, u32, u32);
    /// Check if the virtual display is currently active.
    fn is_active(&self) -> bool;
}

/// Create the appropriate virtual display backend for the current session.
pub fn create_display_backend(session: SessionType) -> Result<Box<dyn VirtualDisplay>> {
    match session {
        SessionType::X11 => Ok(Box::new(x11::X11VirtualDisplay::new()?)),
        SessionType::Wayland => Ok(Box::new(wayland::WaylandVirtualDisplay::new()?)),
        SessionType::Unknown => bail!("Cannot determine display session type"),
    }
}
