//! Host-side input injection using Linux `/dev/uinput` via the `evdev` crate.

use anyhow::{Context, Result};
use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, EventType, InputEvent as RawInputEvent, KeyCode,
    RelativeAxisCode, SynchronizationCode, UinputAbsSetup,
};
use screenextend_common::protocol::{InputEvent, MouseButton};
use tracing::{debug, info};

/// Manages a virtual mouse and keyboard device created through `/dev/uinput`.
pub struct InputInjector {
    device: VirtualDevice,
}

impl InputInjector {
    /// Create and register the virtual input device.
    ///
    /// Requires write permissions to `/dev/uinput` (e.g. membership in the `input` group).
    pub fn new(screen_width: u32, screen_height: u32) -> Result<Self> {
        let mut keys = AttributeSet::<KeyCode>::new();

        // Mouse buttons
        keys.insert(KeyCode::BTN_LEFT);
        keys.insert(KeyCode::BTN_RIGHT);
        keys.insert(KeyCode::BTN_MIDDLE);
        keys.insert(KeyCode::BTN_SIDE);
        keys.insert(KeyCode::BTN_EXTRA);

        // Standard keyboard keys
        for code in 1..=248 {
            keys.insert(KeyCode::new(code));
        }

        // Relative axes (mouse motion & wheel)
        let mut rel_axes = AttributeSet::<RelativeAxisCode>::new();
        rel_axes.insert(RelativeAxisCode::REL_X);
        rel_axes.insert(RelativeAxisCode::REL_Y);
        rel_axes.insert(RelativeAxisCode::REL_WHEEL);
        rel_axes.insert(RelativeAxisCode::REL_HWHEEL);

        // Absolute axes (direct coordinates)
        let abs_x_info = AbsInfo::new(0, 0, screen_width as i32, 0, 0, 1);
        let abs_x_setup = UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, abs_x_info);

        let abs_y_info = AbsInfo::new(0, 0, screen_height as i32, 0, 0, 1);
        let abs_y_setup = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, abs_y_info);

        let device = VirtualDevice::builder()
            .context("Failed to initialize VirtualDevice builder")?
            .name("ScreenExtend Virtual Input")
            .with_keys(&keys)
            .context("Failed to configure keys")?
            .with_relative_axes(&rel_axes)
            .context("Failed to configure relative axes")?
            .with_absolute_axis(&abs_x_setup)
            .context("Failed to configure ABS_X axis")?
            .with_absolute_axis(&abs_y_setup)
            .context("Failed to configure ABS_Y axis")?
            .build()
            .context(
                "Failed to build uinput virtual device. Check permissions for /dev/uinput \
                 (user should be in the 'input' group).",
            )?;

        info!("Created uinput virtual input device: ScreenExtend Virtual Input");

        Ok(Self { device })
    }

    /// Inject an `InputEvent` received from a client.
    pub fn inject(&mut self, event: &InputEvent) -> Result<()> {
        let mut raw_events = Vec::new();

        match event {
            InputEvent::MouseMove { dx, dy } => {
                let rx = *dx as i32;
                let ry = *dy as i32;
                if rx != 0 {
                    raw_events.push(RawInputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_X.0,
                        rx,
                    ));
                }
                if ry != 0 {
                    raw_events.push(RawInputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_Y.0,
                        ry,
                    ));
                }
            }
            InputEvent::MouseMoveAbsolute { x, y } => {
                raw_events.push(RawInputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_X.0,
                    *x as i32,
                ));
                raw_events.push(RawInputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_Y.0,
                    *y as i32,
                ));
            }
            InputEvent::MouseButton { button, pressed } => {
                let key = match button {
                    MouseButton::Left => KeyCode::BTN_LEFT,
                    MouseButton::Right => KeyCode::BTN_RIGHT,
                    MouseButton::Middle => KeyCode::BTN_MIDDLE,
                    MouseButton::Back => KeyCode::BTN_SIDE,
                    MouseButton::Forward => KeyCode::BTN_EXTRA,
                };
                let val = if *pressed { 1 } else { 0 };
                raw_events.push(RawInputEvent::new(EventType::KEY.0, key.code(), val));
            }
            InputEvent::MouseScroll { dx, dy } => {
                if *dy != 0.0 {
                    let val = if *dy > 0.0 { 1 } else { -1 };
                    raw_events.push(RawInputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_WHEEL.0,
                        val,
                    ));
                }
                if *dx != 0.0 {
                    let val = if *dx > 0.0 { 1 } else { -1 };
                    raw_events.push(RawInputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_HWHEEL.0,
                        val,
                    ));
                }
            }
            InputEvent::Key { keycode, pressed } => {
                let val = if *pressed { 1 } else { 0 };
                raw_events.push(RawInputEvent::new(EventType::KEY.0, *keycode as u16, val));
            }
        }

        if !raw_events.is_empty() {
            // Add SYN_REPORT
            raw_events.push(RawInputEvent::new(
                EventType::SYNCHRONIZATION.0,
                SynchronizationCode::SYN_REPORT.0,
                0,
            ));

            self.device
                .emit(&raw_events)
                .context("Failed to emit uinput events")?;
            debug!("Emitted {} input events", raw_events.len());
        }

        Ok(())
    }
}
