//! # ScreenExtend Application Entry Point
//!
//! A high-performance peer-to-peer Linux screen extension utility.

mod application;
mod client_view;
mod host_view;
mod widgets;

use application::ScreenExtendApp;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> gtk4::glib::ExitCode {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "screenextend=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting ScreenExtend application");

    // Initialize GStreamer before GUI loop
    if let Err(e) = gstreamer::init() {
        tracing::error!("Failed to initialize GStreamer: {}", e);
    }

    let app = ScreenExtendApp::new();
    app.run()
}
