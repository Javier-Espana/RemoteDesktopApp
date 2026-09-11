#!/usr/bin/env bash
# Setup script for ScreenExtend development dependencies
# Run: chmod +x scripts/setup_deps.sh && ./scripts/setup_deps.sh

set -euo pipefail

echo "=== ScreenExtend: Installing system dependencies ==="

sudo apt update

sudo apt install -y \
    build-essential \
    pkg-config \
    gstreamer1.0-plugins-bad \
    libgstreamer-plugins-bad1.0-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libgtk-4-dev \
    libadwaita-1-dev \
    libpipewire-0.3-dev \
    libnice-dev \
    libsrtp2-dev \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    gstreamer1.0-pipewire \
    gstreamer1.0-vaapi \
    gstreamer1.0-gl \
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
gst-inspect-1.0 --exists x264enc && echo "✓ x264enc" || echo "✗ x264enc MISSING"
gst-inspect-1.0 --exists opusenc && echo "✓ opusenc" || echo "✗ opusenc MISSING"
gst-inspect-1.0 --exists webrtcbin && echo "✓ webrtcbin" || echo "✗ webrtcbin MISSING"
gst-inspect-1.0 --exists pipewiresrc && echo "✓ pipewiresrc" || echo "✗ pipewiresrc MISSING"
gst-inspect-1.0 --exists ximagesrc && echo "✓ ximagesrc" || echo "✗ ximagesrc MISSING"

echo ""
echo "=== Verifying Rust toolchain ==="
rustc --version
cargo --version

echo ""
echo "=== Setup complete! ==="
echo "Run: cargo build --workspace"
