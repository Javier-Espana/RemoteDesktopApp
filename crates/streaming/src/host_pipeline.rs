//! Host-side GStreamer pipeline for screen capture, encoding, and WebRTC transmission.
//!
//! Pipeline architecture:
//! ```text
//! ximagesrc → videoconvert → x264enc (zerolatency) → rtph264pay → webrtcbin
//! pulsesrc  → audioconvert → opusenc               → rtpopuspay → webrtcbin
//! ```

use anyhow::{Context, Result};
use gstreamer::prelude::*;
use tracing::{debug, error, info};

/// Supported H.264 video encoders with automatic hardware acceleration detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Encoder {
    Nvenc,
    Vaapi,
    SoftwareX264,
}

impl H264Encoder {
    /// Detect the best available video encoder on the system.
    pub fn detect_best() -> Self {
        if gstreamer::ElementFactory::find("nvh264enc").is_some()
            && std::path::Path::new("/dev/nvidia0").exists()
        {
            info!("Hardware video encoder detected: NVIDIA NVENC (nvh264enc)");
            return H264Encoder::Nvenc;
        }

        if gstreamer::ElementFactory::find("vaapih264enc").is_some()
            && std::path::Path::new("/dev/dri/renderD128").exists()
        {
            info!("Hardware video encoder detected: VA-API (vaapih264enc)");
            return H264Encoder::Vaapi;
        }

        info!("Using software video encoder: x264enc (CPU ultrafast zero-latency)");
        H264Encoder::SoftwareX264
    }

    /// Return the GStreamer pipeline string for this encoder.
    pub fn to_pipeline_str(&self, bitrate_kbps: u32, framerate: u32) -> String {
        match self {
            H264Encoder::Nvenc => {
                format!(
                    "nvh264enc bitrate={bitrate} preset=low-latency-hq gop-size={key_int} rc-mode=cbr",
                    bitrate = bitrate_kbps,
                    key_int = framerate
                )
            }
            H264Encoder::Vaapi => {
                format!(
                    "vaapih264enc bitrate={bitrate} rate-control=cbr keyframe-period={key_int}",
                    bitrate = bitrate_kbps,
                    key_int = framerate
                )
            }
            H264Encoder::SoftwareX264 => {
                format!(
                    "x264enc tune=zerolatency speed-preset=ultrafast bitrate={bitrate} \
                     key-int-max={key_int} bframes=0 byte-stream=true",
                    bitrate = bitrate_kbps,
                    key_int = framerate
                )
            }
        }
    }
}

/// Host-side streaming pipeline.
pub struct HostPipeline {
    pipeline: gstreamer::Pipeline,
    webrtcbin: gstreamer::Element,
    _bus_watch: gstreamer::bus::BusWatchGuard,
    encoder: H264Encoder,
}

impl HostPipeline {
    /// Create a new host pipeline with the given capture region and encoding parameters.
    pub fn new(
        startx: u32,
        starty: u32,
        width: u32,
        height: u32,
        framerate: u32,
        bitrate_kbps: u32,
        use_pipewire: bool,
    ) -> Result<Self> {
        gstreamer::init().context("Failed to initialize GStreamer")?;

        let endx = startx + width - 1;
        let endy = starty + height - 1;

        let encoder = H264Encoder::detect_best();
        let encoder_str = encoder.to_pipeline_str(bitrate_kbps, framerate);

        let video_src = if use_pipewire {
            format!(
                "pipewiresrc do-timestamp=true keepalive-time=1000 \
                 ! video/x-raw,framerate={framerate}/1"
            )
        } else {
            format!(
                "ximagesrc display-name=$DISPLAY use-damage=false show-pointer=true \
                 startx={startx} starty={starty} endx={endx} endy={endy} \
                 ! video/x-raw,framerate={framerate}/1"
            )
        };

        // Build the pipeline description
        let pipeline_desc = format!(
            "{video_src} \
             ! videoconvert \
             ! {encoder_str} \
             ! rtph264pay config-interval=-1 pt=96 aggregate-mode=zero-latency \
             ! application/x-rtp,media=video,encoding-name=H264,payload=96 \
             ! webrtcbin name=sendrecv bundle-policy=max-bundle \
             \
             pulsesrc \
             ! audioconvert \
             ! opusenc bitrate=128000 frame-size=10 \
             ! rtpopuspay pt=97 \
             ! application/x-rtp,media=audio,encoding-name=OPUS,payload=97 \
             ! sendrecv.",
            video_src = video_src,
            encoder_str = encoder_str,
        );

        info!("Creating host pipeline with encoder: {:?}", encoder);
        debug!("Pipeline: {}", pipeline_desc);

        let pipeline = gstreamer::parse::launch(&pipeline_desc)
            .context("Failed to parse host pipeline")?
            .downcast::<gstreamer::Pipeline>()
            .map_err(|_| anyhow::anyhow!("Pipeline is not a GstPipeline"))?;

        let webrtcbin = pipeline
            .by_name("sendrecv")
            .context("webrtcbin element 'sendrecv' not found in pipeline")?;

        // Disable STUN/TURN (not needed for LAN)
        webrtcbin.set_property_from_str("stun-server", "");

        // Setup bus error handling
        let bus = pipeline.bus().context("No bus on pipeline")?;
        let pipeline_weak = pipeline.downgrade();
        let bus_watch = bus.add_watch(move |_, msg| {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Error(err) => {
                    error!(
                        "GStreamer error from {:?}: {} ({:?})",
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    if let Some(pipeline) = pipeline_weak.upgrade() {
                        let _ = pipeline.set_state(gstreamer::State::Null);
                    }
                }
                MessageView::Warning(warn) => {
                    debug!(
                        "GStreamer warning: {} ({:?})",
                        warn.error(),
                        warn.debug()
                    );
                }
                MessageView::StateChanged(sc) => {
                    if let Some(element) = sc.src() {
                        if element.type_() == gstreamer::Pipeline::static_type() {
                            info!(
                                "Pipeline state: {:?} → {:?}",
                                sc.old(),
                                sc.current()
                            );
                        }
                    }
                }
                _ => {}
            }
            gstreamer::glib::ControlFlow::Continue
        })
        .context("Failed to add bus watch")?;

        info!("Host pipeline created successfully");

        Ok(Self {
            pipeline,
            webrtcbin,
            _bus_watch: bus_watch,
            encoder,
        })
    }

    /// Get the active encoder type.
    pub fn encoder(&self) -> H264Encoder {
        self.encoder
    }

    /// Dynamically update video bitrate in kbps during active streaming.
    pub fn set_bitrate(&self, bitrate_kbps: u32) -> Result<()> {
        let encoder_names = ["x264enc", "nvh264enc", "vaapih264enc"];
        for element in self.pipeline.iterate_elements() {
            if let Ok(element) = element {
                if let Some(factory) = element.factory() {
                    if encoder_names.iter().any(|name| factory.name() == *name) {
                        element.set_property("bitrate", bitrate_kbps);
                        info!("Dynamically updated encoder bitrate to {} kbps", bitrate_kbps);
                        return Ok(());
                    }
                }
            }
        }
        anyhow::bail!("No active video encoder found in pipeline to update bitrate")
    }

    /// Get a reference to the webrtcbin element (for signaling).
    pub fn webrtcbin(&self) -> &gstreamer::Element {
        &self.webrtcbin
    }

    /// Get a reference to the underlying GStreamer pipeline.
    pub fn pipeline(&self) -> &gstreamer::Pipeline {
        &self.pipeline
    }

    /// Create a WebRTC data channel for input forwarding.
    pub fn create_input_data_channel(&self) -> Result<()> {
        let structure = gstreamer::Structure::builder("data-channel")
            .field("ordered", true)
            .field("max-retransmits", 0i32)
            .field("label", "input")
            .build();

        self.webrtcbin
            .emit_by_name::<Option<gstreamer::glib::Object>>(
                "create-data-channel",
                &[&"input", &structure],
            );

        info!("Created 'input' data channel");
        Ok(())
    }

    /// Start the pipeline (set to Playing state).
    pub fn start(&self) -> Result<()> {
        info!("Starting host pipeline");
        self.pipeline
            .set_state(gstreamer::State::Playing)
            .context("Failed to set pipeline to Playing")?;
        Ok(())
    }

    /// Stop the pipeline (set to Null state).
    pub fn stop(&self) -> Result<()> {
        info!("Stopping host pipeline");
        self.pipeline
            .set_state(gstreamer::State::Null)
            .context("Failed to set pipeline to Null")?;
        Ok(())
    }

    /// Update the capture region dynamically.
    pub fn set_capture_region(
        &self,
        startx: u32,
        starty: u32,
        width: u32,
        height: u32,
    ) -> Result<()> {
        // Find ximagesrc in the pipeline
        let iter = self.pipeline.iterate_sources();
        for element in iter {
            if let Ok(element) = element {
                if element.factory().map(|f| f.name() == "ximagesrc").unwrap_or(false) {
                    element.set_property("startx", startx);
                    element.set_property("starty", starty);
                    element.set_property("endx", startx + width - 1);
                    element.set_property("endy", starty + height - 1);
                    info!("Updated capture region: {}x{} at ({}, {})", width, height, startx, starty);
                    return Ok(());
                }
            }
        }
        anyhow::bail!("ximagesrc element not found in pipeline")
    }
}

impl Drop for HostPipeline {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gstreamer::State::Null);
    }
}
