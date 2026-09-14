//! Wayland display management.
//!
//! Wayland support requires compositor-specific integration:
//! - GNOME/Mutter: D-Bus API for headless outputs
//! - wlroots (Sway/Hyprland): wlr-output-management protocol
//!
//! Wayland compositors do not expose one universal API for creating a monitor.
//! This backend therefore keeps the requested capture surface dimensions while
//! the PipeWire portal supplies the actual screen source to the streaming layer.

use super::VirtualDisplay;
use anyhow::Result;
use tracing::info;

/// Wayland capture surface backed by the desktop portal/PipeWire.
pub struct WaylandVirtualDisplay {
    active: bool,
    width: u32,
    height: u32,
}

impl WaylandVirtualDisplay {
    /// Create a new Wayland display manager.
    pub fn new() -> Result<Self> {
        Ok(Self { active: false, width: 0, height: 0 })
    }
}

impl VirtualDisplay for WaylandVirtualDisplay {
    fn create(&mut self, width: u32, height: u32) -> Result<()> {
        self.width = width;
        self.height = height;
        self.active = true;
        info!(
            "Using Wayland PipeWire capture mode at {}x{}; compositor virtual outputs are not configured",
            width, height
        );
        Ok(())
    }

    fn destroy(&mut self) -> Result<()> {
        Ok(())
    }

    fn capture_region(&self) -> (u32, u32, u32, u32) {
        (0, 0, self.width, self.height)
    }

    fn is_active(&self) -> bool {
        self.active
    }
}
