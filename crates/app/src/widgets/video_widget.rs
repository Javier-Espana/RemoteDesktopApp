//! Video display widget for GTK4.
//!
//! Receives RGBA frames from GStreamer appsink and renders them
//! efficiently using GTK4 `gdk::MemoryTexture` and `gtk4::Picture`.

use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use std::sync::{Arc, Mutex};
use tracing::debug;

/// A GTK4 widget designed to display streaming video frames with low latency.
pub struct VideoWidget {
    container: gtk4::Box,
    picture: gtk4::Picture,
    last_dimensions: Arc<Mutex<Option<(i32, i32)>>>,
}

impl VideoWidget {
    /// Create a new VideoWidget instance.
    pub fn new() -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        let picture = gtk4::Picture::builder()
            .hexpand(true)
            .vexpand(true)
            .can_shrink(true)
            .keep_aspect_ratio(true)
            .build();

        container.append(&picture);

        Self {
            container,
            picture,
            last_dimensions: Arc::new(Mutex::new(None)),
        }
    }

    /// Access the underlying GTK widget to insert into containers.
    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    /// Get the picture widget (to attach gesture / input controllers).
    pub fn picture(&self) -> &gtk4::Picture {
        &self.picture
    }

    /// Render frame with raw RGBA bytes on the GTK main thread.
    pub fn render_frame(&self, width: i32, height: i32, data: Vec<u8>) {
        let bytes = glib::Bytes::from_owned(data);
        let stride = (width * 4) as usize;
        let texture = gdk::MemoryTexture::new(
            width,
            height,
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            stride,
        );
        self.picture.set_paintable(Some(&texture));
        *self.last_dimensions.lock().unwrap() = Some((width, height));
        debug!("Rendered frame: {}x{}", width, height);
    }
}

impl Default for VideoWidget {
    fn default() -> Self {
        Self::new()
    }
}
