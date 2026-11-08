# SoundThis

**Real-time desktop audio streaming for Omarchy Linux** — high-performance, single-binary, and built natively with Rust and Vite.

SoundThis captures the audio playback from your Omarchy desktop (PipeWire/PulseAudio) in real-time and streams it to any web browser on your local network (smartphones, laptops, tablets, smart TVs) with ultra-low latency (~30–50ms) using WebSockets and the Web Audio API.

---

## Highlights

- **Native Rust Server**: Axum + Tokio async WebSocket broadcast server with minimal memory footprint (~1.6 MB RAM) and sub-3ms startup.
- **Embedded Vite Web Client**: Single-file bundled frontend (`TypeScript` + `Vite` + `AudioContext`), compiled and embedded directly inside the 2.9 MB executable. Zero external runtime assets needed.
- **Omarchy Shell Integration**: Native bar widget (`KeyboardPanel` + `BarIconButton`) with live broadcast status, client count, and an interactive popout panel.
- **Enabled / Disabled Control**: Toggle audio sharing on and off directly from the Omarchy bar (right-click to toggle, or use the switch in the panel).
- **Ultra-Low Latency (~30–50ms)**: Streams 48kHz 16-bit stereo PCM with scheduled jitter buffers.
- **Device Identification**: Clients can customize their device name in the browser, stored in `localStorage` and visible on the host desktop.
- **Mise & Cargo Distribution**: Managed with `mise.toml` and standard `cargo install`.

---

## Installation

### Via Cargo (Recommended)

```bash
# Clone the repository
git clone https://github.com/soundthis/soundthis.git
cd soundthis

# Build and install binary + setup Omarchy plugin and systemd service
cargo install --path .
soundthis install
```

### Via Mise

```bash
# Full build and install in one step
mise run install
```

This will:
1. Compile the Vite frontend into `web/dist/index.html`.
2. Compile and install the optimized release binary to `~/.cargo/bin/soundthis`.
3. Set up and start the systemd user service (`soundthis.service`).
4. Install and configure the Omarchy shell plugin in `~/.config/omarchy/plugins/soundthis/`.

---

## Usage

### Web Client
Open the streaming page on any device connected to your network:

```
http://<YOUR_LAN_IP>:9697
```
*(Shown in `soundthis status` and in the Omarchy bar popout)*

### Omarchy Bar Widget
- **Left Click**: Opens the SoundThis control panel showing LAN link, connected listeners, and controls.
- **Right / Middle Click**: Immediately toggles audio sharing between Enabled and Disabled.

### CLI Commands
```bash
soundthis status      # View streaming status and connected clients
soundthis toggle      # Toggle audio sharing on/off
soundthis enable      # Enable audio sharing
soundthis disable     # Disable audio sharing
soundthis open        # Open client page in default browser
soundthis serve       # Run streaming server in foreground
soundthis install     # Configure systemd daemon and Omarchy bar
soundthis uninstall   # Completely remove plugin and service
```

---

## Development & Testing with Mise

The project includes a comprehensive `mise.toml` workflow:

```bash
mise run dev          # Run server in debug mode with Cargo
mise run dev:web      # Run Vite dev server with hot module reloading
mise run build        # Build web bundle and optimized release binary
mise run test         # Run both web (Vitest) and server (Cargo) test suites
mise run test:web     # Run frontend tests
mise run test:server  # Run Rust unit and integration tests
```

---

## Architecture (SOLID Principles)

- **`src/audio/`**:
  - `source.rs`: `AudioSource` trait defining pluggable audio capture backends (`ParecAudioSource` for PipeWire/PulseAudio).
  - `broadcaster.rs`: `AudioBroadcaster` fan-out channel with low-latency frame dropping for lagged clients.
- **`src/state.rs`**: Thread-safe `AppState` managing active connections and streaming toggles.
- **`src/server/`**: Axum HTTP and WebSocket handlers serving embedded assets and binary PCM streams.
- **`src/installer/`**: Validates external templates (`systemd/soundthis.service.template`, `plugin/`) and manages systemd and Omarchy shell registration.
- **`web/`**: TypeScript + Vite client with `AudioStreamPlayer`, `CanvasVisualizer`, and `ClientConfigStore`.
