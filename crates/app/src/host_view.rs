//! Host mode view.
//!
//! Provides controls for starting the screen extension host service,
//! displays the 4-digit PIN for client authentication, and shows streaming metrics.

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use screenextend_common::config::{AppConfig, SessionType};
use screenextend_common::protocol::DEFAULT_SIGNALING_PORT;
use screenextend_discovery::handshake::HandshakeServer;
use screenextend_discovery::mdns::DiscoveryService;
use screenextend_display::create_display_backend;
use screenextend_input::InputInjector;
use screenextend_streaming::host_pipeline::HostPipeline;
use screenextend_streaming::signaling::negotiate_as_host;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub struct HostView {
    container: gtk4::Box,
    status_label: gtk4::Label,
    pin_label: gtk4::Label,
    toggle_btn: gtk4::Button,
    is_running: Arc<AtomicBool>,
}

impl HostView {
    pub fn new() -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        // Header info
        let title = gtk4::Label::builder()
            .label("Host Mode: Extend Display")
            .css_classes(["title-2"])
            .halign(gtk4::Align::Start)
            .build();
        container.append(&title);

        let desc = gtk4::Label::builder()
            .label("Share a virtual screen with another machine on your local network.")
            .css_classes(["dim-label"])
            .halign(gtk4::Align::Start)
            .wrap(true)
            .build();
        container.append(&desc);

        // Status Card
        let status_group = libadwaita::PreferencesGroup::builder()
            .title("Service Status")
            .build();

        let status_row = libadwaita::ActionRow::builder()
            .title("Status")
            .subtitle("Inactive")
            .build();
        let status_label = gtk4::Label::builder()
            .label("Stopped")
            .css_classes(["dim-label"])
            .build();
        status_row.add_suffix(&status_label);
        status_group.add(&status_row);

        // PIN Card
        let pin_row = libadwaita::ActionRow::builder()
            .title("Authentication PIN")
            .subtitle("Enter this 4-digit code on the client machine")
            .build();
        let pin_label = gtk4::Label::builder()
            .label("----")
            .css_classes(["title-1"])
            .build();
        pin_row.add_suffix(&pin_label);
        status_group.add(&pin_row);

        // Session info row
        let session = SessionType::detect();
        let session_row = libadwaita::ActionRow::builder()
            .title("Display Server")
            .subtitle(&format!("Detected session: {}", session))
            .build();
        status_group.add(&session_row);

        container.append(&status_group);

        // Action button
        let toggle_btn = gtk4::Button::builder()
            .label("Start Sharing")
            .css_classes(["suggested-action", "pill"])
            .halign(gtk4::Align::Center)
            .margin_top(12)
            .build();
        container.append(&toggle_btn);

        let is_running = Arc::new(AtomicBool::new(false));

        let view = Self {
            container,
            status_label,
            pin_label,
            toggle_btn,
            is_running,
        };

        view.setup_callbacks();
        view
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    fn setup_callbacks(&self) {
        let is_running = Arc::clone(&self.is_running);
        let toggle_btn = self.toggle_btn.clone();
        let status_label = self.status_label.clone();
        let pin_label = self.pin_label.clone();

        toggle_btn.connect_clicked(move |btn| {
            if is_running.load(Ordering::SeqCst) {
                // Stop service
                is_running.store(false, Ordering::SeqCst);
                btn.set_label("Start Sharing");
                btn.remove_css_class("destructive-action");
                btn.add_css_class("suggested-action");
                status_label.set_label("Stopped");
                pin_label.set_label("----");
                info!("Host service stopped by user");
            } else {
                // Start service
                is_running.store(true, Ordering::SeqCst);
                btn.set_label("Stop Sharing");
                btn.remove_css_class("suggested-action");
                btn.add_css_class("destructive-action");
                status_label.set_label("Starting...");

                let running_flag = Arc::clone(&is_running);
                let btn_clone = btn.clone();
                let status_clone = status_label.clone();
                let pin_clone = pin_label.clone();

                // Launch background async task for mDNS, Handshake, Display and WebRTC
                glib::MainContext::default().spawn_local(async move {
                    if let Err(e) = Self::run_host_service(
                        running_flag.clone(),
                        status_clone.clone(),
                        pin_clone.clone(),
                    )
                    .await
                    {
                        error!("Host service error: {}", e);
                        status_clone.set_label(&format!("Error: {}", e));
                        running_flag.store(false, Ordering::SeqCst);
                        btn_clone.set_label("Start Sharing");
                        btn_clone.remove_css_class("destructive-action");
                        btn_clone.add_css_class("suggested-action");
                    }
                });
            }
        });
    }

    async fn run_host_service(
        running_flag: Arc<AtomicBool>,
        status_label: gtk4::Label,
        pin_label: gtk4::Label,
    ) -> anyhow::Result<()> {
        let port = DEFAULT_SIGNALING_PORT;
        let server = HandshakeServer::new(port).await?;
        let pin = server.pin().to_string();

        pin_label.set_label(&pin);
        status_label.set_label("Broadcasting & Listening...");
        info!("Host service listening on port {} with PIN {}", port, pin);

        // Start mDNS registration
        let (event_tx, _event_rx) = mpsc::channel(10);
        let config = AppConfig::default();
        let mut discovery = DiscoveryService::new(&config.hostname, port, event_tx)?;
        discovery.register(port, true)?;

        // Prepare virtual display backend
        let session = SessionType::detect();
        let mut display = create_display_backend(session)?;
        display.create(config.resolution.width, config.resolution.height)?;
        let (vx, vy, vw, vh) = display.capture_region();

        // Accept connection from client
        let mut channel = server.accept().await?;
        status_label.set_label("Client connected! Streaming...");
        info!("Client connected successfully");

        // Initialize uinput injector for incoming input
        let mut injector = match InputInjector::new(vw, vh) {
            Ok(inj) => Some(inj),
            Err(e) => {
                warn!("Could not create uinput device: {}. Input forwarding disabled.", e);
                None
            }
        };

        // Create GStreamer pipeline
        let pipeline = HostPipeline::new(
            vx,
            vy,
            vw,
            vh,
            config.framerate,
            config.video_bitrate_kbps,
        )?;

        pipeline.create_input_data_channel()?;
        pipeline.start()?;

        // Perform WebRTC signaling
        negotiate_as_host(&mut channel, pipeline.webrtcbin()).await?;

        // Process signaling and input events while running
        while running_flag.load(Ordering::SeqCst) {
            tokio::select! {
                msg = channel.recv() => {
                    match msg {
                        Ok(screenextend_common::protocol::SignalingMessage::SessionControl(
                            screenextend_common::protocol::SessionControlPayload::Disconnect
                        )) => {
                            info!("Client requested disconnect");
                            break;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            warn!("Signaling connection lost: {}", e);
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {}
            }
        }

        pipeline.stop()?;
        display.destroy()?;
        let _ = discovery.shutdown();

        if let Some(_inj) = injector.take() {
            info!("Uinput device cleaned up");
        }

        status_label.set_label("Stopped");
        pin_label.set_label("----");
        Ok(())
    }
}

impl Default for HostView {
    fn default() -> Self {
        Self::new()
    }
}
