//! Client-side GStreamer pipeline for WebRTC reception, decoding, and rendering.
//!
//! Pipeline architecture:
//! ```text
//! webrtcbin → (pad-added) → rtph264depay → avdec_h264 → videoconvert → appsink
//! webrtcbin → (pad-added) → rtpopusdepay → opusdec → audioconvert → autoaudiosink
//! ```
//!
//! The appsink provides decoded video frames to the GTK4 rendering widget.

use anyhow::{Context, Result};
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};

/// Callback type for receiving decoded video frames.
pub type FrameCallback = Arc<Mutex<Option<Box<dyn Fn(gstreamer::Sample) + Send + 'static>>>>;

/// Client-side receiving pipeline.
pub struct ClientPipeline {
    pipeline: gstreamer::Pipeline,
    webrtcbin: gstreamer::Element,
    frame_callback: FrameCallback,
    _bus_watch: gstreamer::bus::BusWatchGuard,
}

impl ClientPipeline {
    /// Create a new client pipeline ready to receive WebRTC streams.
    pub fn new() -> Result<Self> {
        gstreamer::init().context("Failed to initialize GStreamer")?;

        let pipeline = gstreamer::Pipeline::new();

        // Create webrtcbin
        let webrtcbin = gstreamer::ElementFactory::make("webrtcbin")
            .name("recvbin")
            .property_from_str("bundle-policy", "max-bundle")
            .property_from_str("stun-server", "")
            .build()
            .context("Failed to create webrtcbin element")?;

        pipeline.add(&webrtcbin).context("Failed to add webrtcbin to pipeline")?;

        let frame_callback: FrameCallback = Arc::new(Mutex::new(None));

        // Connect to pad-added to dynamically link decoders
        let pipeline_weak = pipeline.downgrade();
        let frame_cb = Arc::clone(&frame_callback);
        webrtcbin.connect_pad_added(move |_webrtcbin, pad| {
            let pipeline = match pipeline_weak.upgrade() {
                Some(p) => p,
                None => return,
            };

            let caps = pad.current_caps().unwrap_or_else(|| pad.query_caps(None));
            if caps.is_empty() {
                warn!("pad-added with no caps, ignoring");
                return;
            }

            let media_type = caps
                .structure(0)
                .and_then(|s| s.get::<&str>("media").ok().map(|m| m.to_string()))
                .unwrap_or_default();

            info!("WebRTC pad added: media={}, caps={}", media_type, caps);

            match media_type.as_str() {
                "video" => {
                    if let Err(e) = Self::link_video_decoder(&pipeline, pad, &frame_cb) {
                        error!("Failed to link video decoder: {}", e);
                    }
                }
                "audio" => {
                    if let Err(e) = Self::link_audio_decoder(&pipeline, pad) {
                        error!("Failed to link audio decoder: {}", e);
                    }
                }
                _ => {
                    debug!("Ignoring pad with media type: {}", media_type);
                }
            }
        });

        // Setup bus error handling
        let bus = pipeline.bus().context("No bus on pipeline")?;
        let bus_watch = bus.add_watch(move |_, msg| {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Error(err) => {
                    error!(
                        "Client GStreamer error: {} ({:?})",
                        err.error(),
                        err.debug()
                    );
                }
                MessageView::Warning(warn) => {
                    debug!("Client GStreamer warning: {}", warn.error());
                }
                MessageView::StateChanged(sc) => {
                    if let Some(element) = sc.src() {
                        if element.type_() == gstreamer::Pipeline::static_type() {
                            info!("Client pipeline state: {:?} → {:?}", sc.old(), sc.current());
                        }
                    }
                }
                _ => {}
            }
            gstreamer::glib::ControlFlow::Continue
        })
        .context("Failed to add bus watch")?;

        info!("Client pipeline created successfully");

        Ok(Self {
            pipeline,
            webrtcbin,
            frame_callback,
            _bus_watch: bus_watch,
        })
    }

    /// Link a video decoder chain to a webrtcbin pad.
    fn link_video_decoder(
        pipeline: &gstreamer::Pipeline,
        pad: &gstreamer::Pad,
        frame_callback: &FrameCallback,
    ) -> Result<()> {
        let depay = gstreamer::ElementFactory::make("rtph264depay")
            .build()
            .context("Failed to create rtph264depay")?;

        let decoder = gstreamer::ElementFactory::make("avdec_h264")
            .build()
            .context("Failed to create avdec_h264")?;

        let convert = gstreamer::ElementFactory::make("videoconvert")
            .build()
            .context("Failed to create videoconvert")?;

        let appsink = gst_app::AppSink::builder()
            .name("videosink")
            .caps(
                &gstreamer_video::VideoCapsBuilder::new()
                    .format(gstreamer_video::VideoFormat::Rgba)
                    .build(),
            )
            .max_buffers(1)
            .drop(true) // Drop old frames if we're too slow
            .sync(false) // Don't sync to clock — render ASAP for low latency
            .build();

        // Set callback for new video frames
        let frame_cb = Arc::clone(frame_callback);
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gstreamer::FlowError::Error)?;

                    if let Some(ref cb) = *frame_cb.lock().unwrap() {
                        cb(sample);
                    }

                    Ok(gstreamer::FlowSuccess::Ok)
                })
                .build(),
        );

        pipeline.add_many([&depay, &decoder, &convert, appsink.upcast_ref()])?;
        gstreamer::Element::link_many([&depay, &decoder, &convert, appsink.upcast_ref()])?;

        depay.sync_state_with_parent()?;
        decoder.sync_state_with_parent()?;
        convert.sync_state_with_parent()?;
        appsink.sync_state_with_parent()?;

        let sink_pad = depay
            .static_pad("sink")
            .context("No sink pad on depay")?;
        pad.link(&sink_pad)
            .context("Failed to link webrtcbin pad to depay")?;

        info!("Video decoder chain linked successfully");
        Ok(())
    }

    /// Link an audio decoder chain to a webrtcbin pad.
    fn link_audio_decoder(
        pipeline: &gstreamer::Pipeline,
        pad: &gstreamer::Pad,
    ) -> Result<()> {
        let depay = gstreamer::ElementFactory::make("rtpopusdepay")
            .build()
            .context("Failed to create rtpopusdepay")?;

        let decoder = gstreamer::ElementFactory::make("opusdec")
            .build()
            .context("Failed to create opusdec")?;

        let convert = gstreamer::ElementFactory::make("audioconvert")
            .build()
            .context("Failed to create audioconvert")?;

        let sink = gstreamer::ElementFactory::make("autoaudiosink")
            .build()
            .context("Failed to create autoaudiosink")?;

        pipeline.add_many([&depay, &decoder, &convert, &sink])?;
        gstreamer::Element::link_many([&depay, &decoder, &convert, &sink])?;

        depay.sync_state_with_parent()?;
        decoder.sync_state_with_parent()?;
        convert.sync_state_with_parent()?;
        sink.sync_state_with_parent()?;

        let sink_pad = depay
            .static_pad("sink")
            .context("No sink pad on audio depay")?;
        pad.link(&sink_pad)
            .context("Failed to link webrtcbin pad to audio depay")?;

        info!("Audio decoder chain linked successfully");
        Ok(())
    }

    /// Get a reference to the webrtcbin element (for signaling).
    pub fn webrtcbin(&self) -> &gstreamer::Element {
        &self.webrtcbin
    }

    /// Get a reference to the underlying GStreamer pipeline.
    pub fn pipeline(&self) -> &gstreamer::Pipeline {
        &self.pipeline
    }

    /// Set a callback to receive decoded video frames.
    ///
    /// The callback receives a `gstreamer::Sample` containing an RGBA frame.
    pub fn set_frame_callback<F>(&self, callback: F)
    where
        F: Fn(gstreamer::Sample) + Send + 'static,
    {
        *self.frame_callback.lock().unwrap() = Some(Box::new(callback));
    }

    /// Start the pipeline.
    pub fn start(&self) -> Result<()> {
        info!("Starting client pipeline");
        self.pipeline
            .set_state(gstreamer::State::Playing)
            .context("Failed to set client pipeline to Playing")?;
        Ok(())
    }

    /// Stop the pipeline.
    pub fn stop(&self) -> Result<()> {
        info!("Stopping client pipeline");
        self.pipeline
            .set_state(gstreamer::State::Null)
            .context("Failed to set client pipeline to Null")?;
        Ok(())
    }
}

impl Drop for ClientPipeline {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gstreamer::State::Null);
    }
}
