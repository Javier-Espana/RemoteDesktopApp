//! WebRTC signaling exchange over the established TCP signaling channel.
//!
//! Bridges the discovery handshake channel with GStreamer's webrtcbin
//! element for SDP offer/answer and ICE candidate exchange.

use anyhow::{Context, Result};
use gstreamer::prelude::*;
use gstreamer_sdp as gst_sdp;
use gstreamer_webrtc as gst_webrtc;
use screenextend_common::protocol::*;
use screenextend_discovery::handshake::SignalingChannel;
use tracing::{debug, error, info};

/// Run WebRTC signaling as the host (offerer).
///
/// Creates the SDP offer from webrtcbin, sends it to the client,
/// receives the answer, and exchanges ICE candidates.
pub async fn negotiate_as_host(
    channel: &mut SignalingChannel,
    webrtcbin: &gstreamer::Element,
) -> Result<()> {
    info!("Starting WebRTC negotiation as host (offerer)");

    // Create offer
    let offer = create_offer(webrtcbin).await?;
    let sdp_text = offer.sdp().to_string();

    // Send SDP offer
    channel
        .send(&SignalingMessage::SdpOffer(SdpPayload {
            sdp_type: "offer".into(),
            sdp: sdp_text,
        }))
        .await
        .context("Failed to send SDP offer")?;

    info!("Sent SDP offer, waiting for answer...");

    // Receive SDP answer
    let answer_sdp = loop {
        match channel.recv().await? {
            SignalingMessage::SdpAnswer(payload) => {
                break payload.sdp;
            }
            SignalingMessage::IceCandidate(ice) => {
                // Client may send ICE candidates before the answer
                apply_ice_candidate(webrtcbin, &ice)?;
            }
            other => {
                debug!("Ignoring message during negotiation: {:?}", other);
            }
        }
    };

    // Apply the remote answer
    apply_answer(webrtcbin, &answer_sdp)?;
    info!("Applied remote SDP answer");

    // Exchange ICE candidates (in background)
    exchange_ice_candidates(channel, webrtcbin).await?;

    Ok(())
}

/// Run WebRTC signaling as the client (answerer).
///
/// Receives the SDP offer from the host, creates an answer,
/// and exchanges ICE candidates.
pub async fn negotiate_as_client(
    channel: &mut SignalingChannel,
    webrtcbin: &gstreamer::Element,
) -> Result<()> {
    info!("Starting WebRTC negotiation as client (answerer)");

    // Receive SDP offer
    let offer_sdp = loop {
        match channel.recv().await? {
            SignalingMessage::SdpOffer(payload) => {
                break payload.sdp;
            }
            other => {
                debug!("Ignoring message while waiting for offer: {:?}", other);
            }
        }
    };

    // Apply the remote offer
    apply_offer(webrtcbin, &offer_sdp)?;
    info!("Applied remote SDP offer");

    // Create answer
    let answer = create_answer(webrtcbin).await?;
    let sdp_text = answer.sdp().to_string();

    // Send SDP answer
    channel
        .send(&SignalingMessage::SdpAnswer(SdpPayload {
            sdp_type: "answer".into(),
            sdp: sdp_text,
        }))
        .await
        .context("Failed to send SDP answer")?;

    info!("Sent SDP answer");

    // Exchange ICE candidates
    exchange_ice_candidates(channel, webrtcbin).await?;

    Ok(())
}

/// Exchange ICE candidates between peers.
///
/// This runs in a loop, sending local candidates and receiving remote ones.
pub async fn exchange_ice_candidates(
    channel: &mut SignalingChannel,
    webrtcbin: &gstreamer::Element,
) -> Result<()> {
    // Set up a channel to receive local ICE candidates from the webrtcbin callback
    let (ice_tx, mut ice_rx) = tokio::sync::mpsc::channel::<IceCandidatePayload>(32);

    // Connect to on-ice-candidate signal
    let ice_tx_clone = ice_tx.clone();
    webrtcbin.connect("on-ice-candidate", false, move |args| {
        let sdp_m_line_index = args[1].get::<u32>().unwrap();
        let candidate = args[2].get::<String>().unwrap();

        debug!("Local ICE candidate: {} (mline={})", candidate, sdp_m_line_index);

        let payload = IceCandidatePayload {
            candidate,
            sdp_m_line_index,
        };

        // Use try_send since we're in a sync callback
        if let Err(e) = ice_tx_clone.try_send(payload) {
            error!("Failed to queue ICE candidate: {}", e);
        }

        None
    });

    // Process ICE candidates bidirectionally
    // We use a short timeout-based loop for the MVP
    let mut rounds = 0;
    loop {
        tokio::select! {
            // Send local ICE candidates
            Some(local_ice) = ice_rx.recv() => {
                channel
                    .send(&SignalingMessage::IceCandidate(local_ice))
                    .await?;
            }
            // Give some time for ICE gathering, then break
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
                rounds += 1;
                if rounds >= 3 {
                    info!("ICE exchange complete after {} rounds", rounds);
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Create an SDP offer from webrtcbin.
async fn create_offer(
    webrtcbin: &gstreamer::Element,
) -> Result<gst_webrtc::WebRTCSessionDescription> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let promise = gstreamer::Promise::with_change_func(move |reply| {
        let reply = match reply {
            Ok(Some(reply)) => reply,
            Ok(None) => {
                let _ = tx.send(Err(anyhow::anyhow!("Empty reply from create-offer")));
                return;
            }
            Err(_) => {
                let _ = tx.send(Err(anyhow::anyhow!("create-offer promise interrupted")));
                return;
            }
        };

        let offer = match reply.value("offer") {
            Ok(offer) => offer,
            Err(e) => {
                let _ = tx.send(Err(anyhow::anyhow!("No offer in reply: {}", e)));
                return;
            }
        };

        let offer = offer
            .get::<gst_webrtc::WebRTCSessionDescription>()
            .expect("offer is not WebRTCSessionDescription");

        let _ = tx.send(Ok(offer));
    });

    webrtcbin.emit_by_name::<()>("create-offer", &[&None::<gstreamer::Structure>, &promise]);

    rx.await.context("create-offer channel closed")?
}

/// Create an SDP answer from webrtcbin.
async fn create_answer(
    webrtcbin: &gstreamer::Element,
) -> Result<gst_webrtc::WebRTCSessionDescription> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let promise = gstreamer::Promise::with_change_func(move |reply| {
        let reply = match reply {
            Ok(Some(reply)) => reply,
            Ok(None) => {
                let _ = tx.send(Err(anyhow::anyhow!("Empty reply from create-answer")));
                return;
            }
            Err(_) => {
                let _ = tx.send(Err(anyhow::anyhow!("create-answer promise interrupted")));
                return;
            }
        };

        let answer = match reply.value("answer") {
            Ok(answer) => answer,
            Err(e) => {
                let _ = tx.send(Err(anyhow::anyhow!("No answer in reply: {}", e)));
                return;
            }
        };

        let answer = answer
            .get::<gst_webrtc::WebRTCSessionDescription>()
            .expect("answer is not WebRTCSessionDescription");

        let _ = tx.send(Ok(answer));
    });

    webrtcbin.emit_by_name::<()>("create-answer", &[&None::<gstreamer::Structure>, &promise]);

    rx.await.context("create-answer channel closed")?
}

/// Apply a remote SDP offer to webrtcbin.
fn apply_offer(webrtcbin: &gstreamer::Element, sdp_str: &str) -> Result<()> {
    let sdp =
        gst_sdp::SDPMessage::parse_buffer(sdp_str.as_bytes()).context("Failed to parse SDP offer")?;
    let offer =
        gst_webrtc::WebRTCSessionDescription::new(gst_webrtc::WebRTCSDPType::Offer, sdp);

    webrtcbin.emit_by_name::<()>("set-remote-description", &[&offer, &None::<gstreamer::Promise>]);

    Ok(())
}

/// Apply a remote SDP answer to webrtcbin.
fn apply_answer(webrtcbin: &gstreamer::Element, sdp_str: &str) -> Result<()> {
    let sdp = gst_sdp::SDPMessage::parse_buffer(sdp_str.as_bytes())
        .context("Failed to parse SDP answer")?;
    let answer =
        gst_webrtc::WebRTCSessionDescription::new(gst_webrtc::WebRTCSDPType::Answer, sdp);

    webrtcbin.emit_by_name::<()>("set-remote-description", &[&answer, &None::<gstreamer::Promise>]);

    Ok(())
}

/// Apply a remote ICE candidate to webrtcbin.
fn apply_ice_candidate(
    webrtcbin: &gstreamer::Element,
    ice: &IceCandidatePayload,
) -> Result<()> {
    debug!(
        "Applying remote ICE candidate: {} (mline={})",
        ice.candidate, ice.sdp_m_line_index
    );

    webrtcbin.emit_by_name::<()>(
        "add-ice-candidate",
        &[&ice.sdp_m_line_index, &ice.candidate],
    );

    Ok(())
}
