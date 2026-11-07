# SoundThis

**Real-time desktop audio streaming for Omarchy Linux** — like SoundIt, but minimal, native, and built for Omarchy.

SoundThis captures the audio playback from your Omarchy desktop (PipeWire/PulseAudio) in real-time and streams it to any web browser on your local network (smartphones, laptops, tablets, smart TVs) with ultra-low latency (~30–50ms) using Web Audio API and WebSockets.

---

## Features

- **Omarchy Shell Integration**: Native bar widget with live broadcast status, client count, and an interactive popout panel.
- **Enabled / Disabled Control**: Toggle audio sharing on and off directly from the Omarchy bar (right-click to toggle, or use the switch in the panel).
- **Ultra-Low Latency (~30–50ms)**: Uses raw 48kHz 16-bit stereo PCM streaming directly into browser Web Audio API jitter buffers — no transcoding lag.
- **Device Identification**: Clients can customize their device name in the browser, which is stored in `localStorage` and visible on the host desktop.
- **Universal Browser Support**: Works out of the box on mobile Safari (iOS), Chrome (Android/Desktop), Firefox, and Edge.
- **Open Access**: Supports HTTP with all origins (`Access-Control-Allow-Origin: *`) on port **9697**.
- **Zero Heavy Dependencies**: Pure Python 3 async backend using standard library and PipeWire monitor capture.

---

## Installation

Run the installer from this repository:

```bash
./install.sh
```

This will:
1. Install `soundthis` to `~/.local/bin/soundthis`.
2. Install the Omarchy shell plugin to `~/.config/omarchy/plugins/soundthis/`.
3. Enable the widget in your Omarchy bar (`--section right`).
4. Enable and start the systemd user service (`soundthis.service`).

---

## Usage

### Web Client
Open the streaming page on any device connected to your network:

```
http://<YOUR_LAN_IP>:9697
```
(Or on the host machine: `http://localhost:9697`)

1. Tap **Start Listening**.
2. Customize your device name (e.g. "Living Room TV", "iPhone"). It automatically persists in local storage!

### Omarchy Bar Widget
- **Left Click**: Opens the SoundThis control panel showing LAN link, connected listeners, and controls.
- **Right / Middle Click**: Immediately toggles audio sharing between Enabled and Disabled.

### CLI Commands
```bash
soundthis status      # View current streaming status and connected clients
soundthis toggle      # Toggle audio sharing on/off
soundthis enable      # Enable audio sharing
soundthis disable     # Disable audio sharing
soundthis open        # Open client page in default browser
soundthis serve       # Run streaming server in foreground
soundthis uninstall   # Completely remove plugin and service
```

---

## Uninstallation

To completely remove SoundThis from your system:

```bash
./uninstall.sh
# or: soundthis uninstall
```

This safely stops and removes the background service, deletes the Omarchy shell plugin from your bar, and removes installed application files.

---

## Architecture

- **Capture**: `parec -r -d @DEFAULT_MONITOR@ --rate=48000 --format=s16le --channels=2 --latency-msec=20`
- **Transport**: WebSockets with binary RFC 6455 frames (and chunked HTTP at `/stream.wav`).
- **Playback**: Web Audio API (`AudioContext`) with scheduled jitter buffer and dynamic resampling.
- **Shell Plugin**: Built with Quickshell, `BarIconButton`, and `KeyboardPanel` following Omarchy design tokens.
