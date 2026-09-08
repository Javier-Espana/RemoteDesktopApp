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

    // Build a Tokio multi-thread runtime and keep it alive for the entire
    // process lifetime. GTK must own the main thread; we enter the runtime
    // so that any future awaited in `glib::spawn_local` or background task
    // finds a reactor via Tokio context.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    let _guard = rt.enter();

    let app = ScreenExtendApp::new();
    let exit = app.run();

    rt.shutdown_background();
    exit
}
