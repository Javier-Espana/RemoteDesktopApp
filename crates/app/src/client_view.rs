//! Client mode view.
//!
//! Discovers host machines via mDNS, accepts PIN authentication,
//! renders incoming video frames, and forwards user input.

use crate::widgets::VideoWidget;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use screenextend_common::config::AppConfig;
use screenextend_common::protocol::DEFAULT_SIGNALING_PORT;
use screenextend_discovery::handshake::HandshakeClient;
use screenextend_discovery::mdns::{DiscoveryEvent, DiscoveryService};
use screenextend_input::InputCapture;
use screenextend_streaming::client_pipeline::ClientPipeline;
use screenextend_streaming::signaling::negotiate_as_client;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub struct ClientView {
    container: gtk4::Box,
    video_widget: Rc<VideoWidget>,
    stack: gtk4::Stack,
    ip_entry: gtk4::Entry,
    pin_entry: gtk4::Entry,
    connect_btn: gtk4::Button,
    status_label: gtk4::Label,
    peers_list: gtk4::ListBox,
    is_connected: Arc<AtomicBool>,
}

impl ClientView {
    pub fn new() -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        let stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .hexpand(true)
            .vexpand(true)
            .build();

        // Page 1: Connection setup page
        let setup_page = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        let title = gtk4::Label::builder()
            .label("Client Mode: Connect to Host")
            .css_classes(["title-2"])
            .halign(gtk4::Align::Start)
            .build();
        setup_page.append(&title);

        let desc = gtk4::Label::builder()
            .label("Select a discovered host on your network or enter its IP address directly.")
            .css_classes(["dim-label"])
            .halign(gtk4::Align::Start)
            .wrap(true)
            .build();
        setup_page.append(&desc);

        // Discovered hosts group
        let peers_group = libadwaita::PreferencesGroup::builder()
            .title("Discovered Hosts (mDNS)")
            .build();

        let peers_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        peers_group.add(&peers_list);
        setup_page.append(&peers_group);

        // Connection form
        let form_group = libadwaita::PreferencesGroup::builder()
            .title("Connection Details")
            .build();

        let ip_row = libadwaita::ActionRow::builder()
            .title("Host IP Address")
            .build();
        let ip_entry = gtk4::Entry::builder()
            .placeholder_text("192.168.1.xxx or 127.0.0.1")
            .text("127.0.0.1")
            .halign(gtk4::Align::End)
            .build();
        ip_row.add_suffix(&ip_entry);
        form_group.add(&ip_row);

        let pin_row = libadwaita::ActionRow::builder()
            .title("4-Digit PIN")
            .build();
        let pin_entry = gtk4::Entry::builder()
            .placeholder_text("0000")
            .max_length(4)
            .halign(gtk4::Align::End)
            .build();
        pin_row.add_suffix(&pin_entry);
        form_group.add(&pin_row);

        setup_page.append(&form_group);

        let status_label = gtk4::Label::builder()
            .label("Idle")
            .css_classes(["dim-label"])
            .build();
        setup_page.append(&status_label);

        let connect_btn = gtk4::Button::builder()
            .label("Connect")
            .css_classes(["suggested-action", "pill"])
            .halign(gtk4::Align::Center)
            .margin_top(8)
            .build();
        setup_page.append(&connect_btn);

        stack.add_named(&setup_page, Some("setup"));

        // Page 2: Streaming video viewport
        let video_page = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        let video_widget = Rc::new(VideoWidget::new());
        video_page.append(video_widget.widget());

        // Floating disconnect button header bar
        let disconnect_bar = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .margin_start(8)
            .margin_end(8)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        let back_btn = gtk4::Button::builder()
            .label("Disconnect")
            .css_classes(["destructive-action", "flat"])
            .build();
        disconnect_bar.append(&back_btn);
        video_page.prepend(&disconnect_bar);

        stack.add_named(&video_page, Some("stream"));
        container.append(&stack);

        let is_connected = Arc::new(AtomicBool::new(false));

        let view = Self {
            container,
            video_widget,
            stack,
            ip_entry,
            pin_entry,
            connect_btn,
            status_label,
            peers_list,
            is_connected,
        };

        view.setup_events(back_btn);
        view.start_discovery();
        view
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    fn start_discovery(&self) {
        let (tx, mut rx) = mpsc::channel(20);
        let config = AppConfig::default();

        if let Ok(discovery) = DiscoveryService::new(&config.hostname, DEFAULT_SIGNALING_PORT, tx) {
            let _ = discovery.start_browsing();

            let peers_list_clone = self.peers_list.clone();
            let ip_entry_clone = self.ip_entry.clone();

            glib::MainContext::default().spawn_local(async move {
                while let Some(event) = rx.recv().await {
                    match event {
                        DiscoveryEvent::PeerDiscovered(peer) => {
                            if peer.is_host {
                                let row = libadwaita::ActionRow::builder()
                                    .title(&peer.hostname)
                                    .subtitle(&peer.addr.ip().to_string())
                                    .activatable(true)
                                    .build();

                                let ip_str = peer.addr.ip().to_string();
                                let entry = ip_entry_clone.clone();
                                row.connect_activated(move |_| {
                                    entry.set_text(&ip_str);
                                });

                                peers_list_clone.append(&row);
                            }
                        }
                        DiscoveryEvent::PeerLost(_) => {}
                    }
                }
            });
        }
    }

    fn setup_events(&self, back_btn: gtk4::Button) {
        let is_connected = Arc::clone(&self.is_connected);
        let stack = self.stack.clone();
        let ip_entry = self.ip_entry.clone();
        let pin_entry = self.pin_entry.clone();
        let status_label = self.status_label.clone();
        let video_widget = Rc::clone(&self.video_widget);

        // Input forwarding setup
        let (input_tx, input_rx) = mpsc::channel(100);
        let input_rx = Arc::new(tokio::sync::Mutex::new(input_rx));
        let input_capture = InputCapture::new(input_tx);

        // Attach mouse motion controller to video picture
        let motion_ctrl = gtk4::EventControllerMotion::new();
        let cap_motion = input_capture.clone();
        motion_ctrl.connect_motion(move |_, x, y| {
            cap_motion.handle_mouse_absolute(x, y);
        });
        video_widget.picture().add_controller(motion_ctrl);

        // Attach click controller
        let gesture_click = gtk4::GestureClick::new();
        let cap_click_pressed = input_capture.clone();
        gesture_click.connect_pressed(move |gesture, _n, _x, _y| {
            let button = gesture.current_button();
            cap_click_pressed.handle_mouse_button(button, true);
        });
        let cap_click_released = input_capture.clone();
        gesture_click.connect_released(move |gesture, _n, _x, _y| {
            let button = gesture.current_button();
            cap_click_released.handle_mouse_button(button, false);
        });
        video_widget.picture().add_controller(gesture_click);

        // Attach scroll controller
        let scroll_ctrl = gtk4::EventControllerScroll::new(
            gtk4::EventControllerScrollFlags::VERTICAL | gtk4::EventControllerScrollFlags::HORIZONTAL,
        );
        let cap_scroll = input_capture.clone();
        scroll_ctrl.connect_scroll(move |_, dx, dy| {
            cap_scroll.handle_scroll(dx, dy);
            glib::Propagation::Proceed
        });
        video_widget.picture().add_controller(scroll_ctrl);

        // Attach keyboard controller
        let key_ctrl = gtk4::EventControllerKey::new();
        let cap_key_pressed = input_capture.clone();
        key_ctrl.connect_key_pressed(move |_, keyval, keycode, _| {
            cap_key_pressed.handle_key(keycode, true);
            debug!("Key pressed: keyval={}, keycode={}", keyval, keycode);
            glib::Propagation::Proceed
        });
        let cap_key_released = input_capture.clone();
        key_ctrl.connect_key_released(move |_, _keyval, keycode, _| {
            cap_key_released.handle_key(keycode, false);
        });
        video_widget.picture().add_controller(key_ctrl);

        // Connect button
        let is_conn = Arc::clone(&is_connected);
        let stack_clone = stack.clone();
        let vw_clone = Rc::clone(&video_widget);
        let input_rx_clone = Arc::clone(&input_rx);

        self.connect_btn.connect_clicked(move |_| {
            let ip = ip_entry.text().to_string();
            let pin = pin_entry.text().to_string();

            if pin.len() != 4 {
                status_label.set_label("Error: PIN must be 4 digits");
                return;
            }

            status_label.set_label("Connecting...");
            let status = status_label.clone();
            let is_conn_inner = Arc::clone(&is_conn);
            let stack_inner = stack_clone.clone();
            let vw_inner = Rc::clone(&vw_clone);
            let input_rx_inner = Arc::clone(&input_rx_clone);

            glib::MainContext::default().spawn_local(async move {
                let addr_str = format!("{}:{}", ip, DEFAULT_SIGNALING_PORT);
                let addr: SocketAddr = match addr_str.parse() {
                    Ok(a) => a,
                    Err(e) => {
                        status.set_label(&format!("Invalid IP: {}", e));
                        return;
                    }
                };

                // Perform handshake
                match HandshakeClient::connect(addr, &pin).await {
                    Ok(mut channel) => {
                        info!("Handshake complete. Initializing client streaming pipeline...");
                        status.set_label("Handshake OK! Setting up WebRTC...");

                        let pipeline = match ClientPipeline::new() {
                            Ok(p) => p,
                            Err(e) => {
                                status.set_label(&format!("Pipeline error: {}", e));
                                return;
                            }
                        };

                        // Forward frames to VideoWidget via async channel (drops frames if UI is busy)
                        let (frame_tx, mut frame_rx) =
                            tokio::sync::mpsc::channel::<(i32, i32, Vec<u8>)>(2);
                        let vw_for_frames = Rc::clone(&vw_inner);
                        glib::MainContext::default().spawn_local(async move {
                            while let Some((w, h, data)) = frame_rx.recv().await {
                                vw_for_frames.render_frame(w, h, data);
                            }
                        });

                        pipeline.set_frame_callback(move |sample| {
                            if let Some(caps) = sample.caps() {
                                if let Some(s) = caps.structure(0) {
                                    let width = s.get::<i32>("width").unwrap_or(0);
                                    let height = s.get::<i32>("height").unwrap_or(0);
                                    if width > 0 && height > 0 {
                                        if let Some(buf) = sample.buffer() {
                                            if let Ok(map) = buf.map_readable() {
                                                let _ = frame_tx.try_send((width, height, map.as_slice().to_vec()));
                                            }
                                        }
                                    }
                                }
                            }
                        });

                        if let Err(e) = pipeline.start() {
                            status.set_label(&format!("Failed to start pipeline: {}", e));
                            return;
                        }

                        // WebRTC signaling
                        if let Err(e) = negotiate_as_client(&mut channel, pipeline.webrtcbin()).await {
                            status.set_label(&format!("WebRTC negotiation error: {}", e));
                            let _ = pipeline.stop();
                            return;
                        }

                        is_conn_inner.store(true, Ordering::SeqCst);
                        stack_inner.set_visible_child_name("stream");
                        info!("Connected to host. Streaming started.");

                        // Forward captured input to host
                        let mut rx_guard = input_rx_inner.lock().await;
                        while is_conn_inner.load(Ordering::SeqCst) {
                            tokio::select! {
                                Some(input_ev) = rx_guard.recv() => {
                                    let _ = channel.send(&screenextend_common::protocol::SignalingMessage::Input(input_ev)).await;
                                }
                                msg = channel.recv() => {
                                    match msg {
                                        Ok(screenextend_common::protocol::SignalingMessage::SessionControl(
                                            screenextend_common::protocol::SessionControlPayload::Disconnect
                                        )) => {
                                            info!("Host disconnected");
                                            break;
                                        }
                                        Ok(_) => {}
                                        Err(e) => {
                                            tracing::warn!("Signaling channel closed: {}", e);
                                            break;
                                        }
                                    }
                                }
                                _ = tokio::time::sleep(tokio::time::Duration::from_millis(5)) => {}
                            }
                        }

                        let _ = pipeline.stop();
                        is_conn_inner.store(false, Ordering::SeqCst);
                        stack_inner.set_visible_child_name("setup");
                        status.set_label("Disconnected");
                    }
                    Err(e) => {
                        error!("Connection failed: {}", e);
                        status.set_label(&format!("Auth failed: {}", e));
                    }
                }
            });
        });

        // Disconnect button
        let is_conn_disc = Arc::clone(&is_connected);
        back_btn.connect_clicked(move |_| {
            is_conn_disc.store(false, Ordering::SeqCst);
            stack.set_visible_child_name("setup");
            info!("Disconnected from host");
        });
    }
}

impl Default for ClientView {
    fn default() -> Self {
        Self::new()
    }
}
