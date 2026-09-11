//! Wayland virtual display management (placeholder for future implementation).
//!
//! Wayland support requires compositor-specific integration:
//! - GNOME/Mutter: D-Bus API for headless outputs
//! - wlroots (Sway/Hyprland): wlr-output-management protocol
//!
//! For now, this module returns an error indicating Wayland support is not yet implemented.

use super::VirtualDisplay;
use anyhow::{bail, Result};
use tracing::warn;

/// Wayland virtual display (placeholder).
pub struct WaylandVirtualDisplay;

impl WaylandVirtualDisplay {
    /// Create a new Wayland virtual display manager.
    pub fn new() -> Result<Self> {
        warn!("Wayland virtual display support is not yet implemented");
        Ok(Self)
    }
}

impl VirtualDisplay for WaylandVirtualDisplay {
    fn create(&mut self, _width: u32, _height: u32) -> Result<()> {
        bail!(
            "Wayland virtual display is not yet implemented. \
             Please use X11 session or contribute Wayland support."
        )
    }

    fn destroy(&mut self) -> Result<()> {
        Ok(())
    }

    fn capture_region(&self) -> (u32, u32, u32, u32) {
        (0, 0, 1920, 1080)
    }

    fn is_active(&self) -> bool {
        false
    }
}
