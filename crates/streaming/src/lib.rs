//! # ScreenExtend Streaming
//!
//! GStreamer-based video and audio streaming pipelines using WebRTC.
//! Handles screen capture, encoding, transport, decoding, and rendering.

pub mod client_pipeline;
pub mod host_pipeline;
pub mod signaling;

pub use client_pipeline::ClientPipeline;
pub use host_pipeline::HostPipeline;
