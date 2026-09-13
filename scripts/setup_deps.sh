#!/usr/bin/env bash
# Setup script for ScreenExtend development dependencies
# Run: chmod +x scripts/setup_deps.sh && ./scripts/setup_deps.sh

set -euo pipefail

echo "=== ScreenExtend: Installing system dependencies ==="

sudo apt update

sudo apt install -y \
    build-essential \
    rustc \
    cargo \
    pkg-config \
    gstreamer1.0-plugins-bad \
    libgstreamer-plugins-bad1.0-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libgtk-4-dev \
    libadwaita-1-dev \
    libpipewire-0.3-dev \
    libnice-dev \
    gstreamer1.0-nice \
    libsrtp2-dev \
    libx11-dev \
    x11-xserver-utils \
    xcvt \
    libssl-dev \
    curl \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    gstreamer1.0-pipewire \
    gstreamer1.0-vaapi \
    gstreamer1.0-gl \
    gstreamer1.0-x \
    gstreamer1.0-alsa \
    libsdl2-dev \
    avahi-daemon

echo ""
echo "=== Setting up uinput permissions ==="
if [ ! -f /etc/udev/rules.d/99-screenextend-uinput.rules ]; then
    sudo cp config/99-screenextend-uinput.rules /etc/udev/rules.d/
    sudo udevadm control --reload-rules
    sudo udevadm trigger
    echo "uinput rules installed. You may need to log out and back in."
else
    echo "uinput rules already installed."
fi

# Ensure user is in input group
if ! groups "$USER" | grep -q '\binput\b'; then
    sudo usermod -aG input "$USER"
    echo "Added $USER to input group. Please log out and back in."
else
    echo "User already in input group."
fi

echo ""
echo "=== Verifying GStreamer plugins ==="
gst-inspect-1.0 --exists x264enc && echo "✓ x264enc (H.264 software encoder)" || echo "✗ x264enc MISSING"
gst-inspect-1.0 --exists avdec_h264 && echo "✓ avdec_h264 (H.264 software decoder)" || echo "✗ avdec_h264 MISSING"
gst-inspect-1.0 --exists opusenc && echo "✓ opusenc (Audio encoder)" || echo "✗ opusenc MISSING"
gst-inspect-1.0 --exists opusdec && echo "✓ opusdec (Audio decoder)" || echo "✗ opusdec MISSING"
gst-inspect-1.0 --exists webrtcbin && echo "✓ webrtcbin (WebRTC transport)" || echo "✗ webrtcbin MISSING"
gst-inspect-1.0 --exists nicesink && echo "✓ nicesink (libnice ICE agent)" || echo "✗ nicesink (gstreamer1.0-nice) MISSING"
gst-inspect-1.0 --exists pipewiresrc && echo "✓ pipewiresrc (Wayland screen capture)" || echo "✗ pipewiresrc MISSING"
gst-inspect-1.0 --exists ximagesrc && echo "✓ ximagesrc (X11 screen capture)" || echo "✗ ximagesrc MISSING"
gst-inspect-1.0 --exists pulsesrc && echo "✓ pulsesrc (PulseAudio capture)" || echo "✗ pulsesrc MISSING"
gst-inspect-1.0 --exists autoaudiosink && echo "✓ autoaudiosink (Audio playback)" || echo "✗ autoaudiosink MISSING"

echo ""
echo "=== Verifying Display & Network Tools ==="
which xrandr >/dev/null && echo "✓ xrandr" || echo "✗ xrandr MISSING"
which cvt >/dev/null && echo "✓ cvt" || echo "✗ cvt MISSING"
systemctl is-active --quiet avahi-daemon && echo "✓ avahi-daemon (active)" || echo "⚠ avahi-daemon not active (mDNS may be limited)"
[ -c /dev/uinput ] && echo "✓ /dev/uinput exists" || echo "⚠ /dev/uinput missing (run: sudo modprobe uinput)"

echo ""
echo "=== Verifying Rust toolchain ==="
rustc --version
cargo --version

echo ""
echo "=== Setup complete! ==="
echo "Run: cargo build --workspace"
