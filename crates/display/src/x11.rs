//! X11 virtual display management using xrandr.
//!
//! For the MVP, this module provides two strategies:
//! 1. **Region capture** (default): Captures a region of the existing desktop
//!    positioned to the right of the primary monitor. No kernel modules needed.
//! 2. **Virtual output** (future): Uses xrandr with VIRTUAL or evdi outputs
//!    to create a true separate virtual monitor.

use super::VirtualDisplay;
use anyhow::{Context, Result};
use std::process::Command;
use tracing::{info, warn, debug};

/// X11 virtual display using xrandr.
pub struct X11VirtualDisplay {
    /// Whether a virtual display is currently active.
    active: bool,
    /// Primary monitor width (used to position virtual display to the right).
    primary_width: u32,
    /// Primary monitor height.
    primary_height: u32,
    /// Virtual display width.
    virtual_width: u32,
    /// Virtual display height.
    virtual_height: u32,
    /// Name of the virtual output (if using xrandr VIRTUAL output).
    virtual_output: Option<String>,
    /// Custom modeline name (if we created one).
    modeline_name: Option<String>,
}

impl X11VirtualDisplay {
    /// Create a new X11 virtual display manager.
    pub fn new() -> Result<Self> {
        let (primary_width, primary_height) = detect_primary_resolution()?;
        info!(
            "Detected primary monitor: {}x{}",
            primary_width, primary_height
        );

        Ok(Self {
            active: false,
            primary_width,
            primary_height,
            virtual_width: 0,
            virtual_height: 0,
            virtual_output: None,
            modeline_name: None,
        })
    }

    /// Try to find a disconnected output that can be used as a virtual display.
    fn find_disconnected_output() -> Option<String> {
        let output = Command::new("xrandr")
            .arg("--query")
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains(" disconnected") {
                let name = line.split_whitespace().next()?;
                // Prefer VIRTUAL or DUMMY outputs, then DP/HDMI
                if name.starts_with("VIRTUAL")
                    || name.starts_with("DUMMY")
                    || name.starts_with("DP")
                    || name.starts_with("HDMI")
                {
                    debug!("Found disconnected output: {}", name);
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    /// Attempt to create a virtual display using a disconnected xrandr output.
    fn try_xrandr_virtual(&mut self, width: u32, height: u32) -> Result<bool> {
        let output_name = match Self::find_disconnected_output() {
            Some(name) => name,
            None => {
                info!("No disconnected output found, will use region capture mode");
                return Ok(false);
            }
        };

        info!("Attempting to use disconnected output: {}", output_name);

        // Generate a modeline using cvt
        let cvt = Command::new("cvt")
            .arg(width.to_string())
            .arg(height.to_string())
            .arg("60")
            .output()
            .context("Failed to run cvt")?;

        let cvt_output = String::from_utf8_lossy(&cvt.stdout);
        let modeline = cvt_output
            .lines()
            .find(|l| l.starts_with("Modeline"))
            .context("No Modeline in cvt output")?;

        // Parse: Modeline "1920x1080_60.00"  173.00  1920 2048 ...
        let parts: Vec<&str> = modeline.split_whitespace().collect();
        if parts.len() < 3 {
            warn!("Unexpected cvt output format");
            return Ok(false);
        }

        let mode_name = parts[1].trim_matches('"').to_string();
        let mode_params: Vec<&str> = parts[2..].to_vec();

        // Create the new mode
        let mut newmode_args = vec!["--newmode", &mode_name];
        for p in &mode_params {
            newmode_args.push(p);
        }

        let status = Command::new("xrandr")
            .args(&newmode_args)
            .status()
            .context("Failed to run xrandr --newmode")?;

        if !status.success() {
            warn!("xrandr --newmode failed (mode may already exist)");
            // Try anyway — mode might already exist
        }

        // Add mode to the output
        let status = Command::new("xrandr")
            .args(["--addmode", &output_name, &mode_name])
            .status()
            .context("Failed to run xrandr --addmode")?;

        if !status.success() {
            warn!("xrandr --addmode failed");
            return Ok(false);
        }

        // Enable the output to the right of the primary
        let position = format!("{}x0", self.primary_width);
        let status = Command::new("xrandr")
            .args([
                "--output",
                &output_name,
                "--mode",
                &mode_name,
                "--pos",
                &position,
            ])
            .status()
            .context("Failed to enable xrandr output")?;

        if !status.success() {
            warn!("Failed to enable output {}", output_name);
            // Cleanup
            let _ = Command::new("xrandr")
                .args(["--delmode", &output_name, &mode_name])
                .status();
            return Ok(false);
        }

        info!(
            "Created virtual display on {} at position {}",
            output_name, position
        );

        self.virtual_output = Some(output_name);
        self.modeline_name = Some(mode_name);
        Ok(true)
    }
}

impl VirtualDisplay for X11VirtualDisplay {
    fn create(&mut self, width: u32, height: u32) -> Result<()> {
        self.virtual_width = width;
        self.virtual_height = height;

        // Try to create a real virtual output first
        match self.try_xrandr_virtual(width, height) {
            Ok(true) => {
                info!("Using xrandr virtual output mode");
            }
            Ok(false) => {
                info!(
                    "Using region capture mode: capturing {}x{} at ({}, 0)",
                    width, height, self.primary_width
                );
            }
            Err(e) => {
                warn!(
                    "xrandr virtual output failed ({}), falling back to region capture",
                    e
                );
            }
        }

        self.active = true;
        Ok(())
    }

    fn destroy(&mut self) -> Result<()> {
        if let Some(ref output) = self.virtual_output {
            info!("Removing virtual display on {}", output);

            // Disable the output
            let _ = Command::new("xrandr")
                .args(["--output", output, "--off"])
                .status();

            // Remove the mode from the output
            if let Some(ref mode) = self.modeline_name {
                let _ = Command::new("xrandr")
                    .args(["--delmode", output, mode])
                    .status();

                // Remove the mode entirely
                let _ = Command::new("xrandr")
                    .args(["--rmmode", mode])
                    .status();
            }

            self.virtual_output = None;
            self.modeline_name = None;
        }

        self.active = false;
        info!("Virtual display destroyed");
        Ok(())
    }

    fn capture_region(&self) -> (u32, u32, u32, u32) {
        if self.virtual_output.is_some() {
            // If we have a real virtual output, capture from its position
            (self.primary_width, 0, self.virtual_width, self.virtual_height)
        } else {
            // Region capture mode: capture from the right edge of the primary
            // For MVP, we capture the primary screen content
            (0, 0, self.virtual_width.min(self.primary_width), self.virtual_height.min(self.primary_height))
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for X11VirtualDisplay {
    fn drop(&mut self) {
        if self.active {
            if let Err(e) = self.destroy() {
                warn!("Error destroying virtual display on drop: {}", e);
            }
        }
    }
}

/// Detect the primary monitor resolution using xrandr.
fn detect_primary_resolution() -> Result<(u32, u32)> {
    let output = Command::new("xrandr")
        .arg("--query")
        .output()
        .context("Failed to run xrandr")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Look for the primary monitor line or the first connected monitor
    for line in stdout.lines() {
        if line.contains(" connected") {
            // Format: "eDP-1 connected primary 1920x1080+0+0 ..."
            // or: "HDMI-1 connected 2560x1440+1920+0 ..."
            for word in line.split_whitespace() {
                if let Some((res, _pos)) = word.split_once('+') {
                    if let Some((w, h)) = res.split_once('x') {
                        if let (Ok(width), Ok(height)) = (w.parse::<u32>(), h.parse::<u32>()) {
                            return Ok((width, height));
                        }
                    }
                }
            }
        }
    }

    // Fallback
    warn!("Could not detect primary resolution, defaulting to 1920x1080");
    Ok((1920, 1080))
}
