# SoundThis

**Real-time desktop audio streaming for Omarchy Linux** — high-performance, single-binary, and built natively with Rust and Vite.

SoundThis captures the audio playback from your Omarchy desktop (PipeWire/PulseAudio) in real-time and streams it to any web browser on your local network (smartphones, laptops, tablets, smart TVs) with ultra-low latency (~30–50ms) using WebSockets and the Web Audio API.

---

## Highlights

- **Native Rust Server**: Axum + Tokio async WebSocket broadcast server with minimal memory footprint (~1.6 MB RAM) and sub-3ms startup.
- **Embedded Vite Web Client**: Single-file bundled frontend (`TypeScript` + `Vite` + `AudioContext`), compiled and embedded directly inside the 2.9 MB executable. Zero external runtime assets needed.
- **TOML Configuration Support**: Fully customizable via `~/.config/soundthis/config.toml` or custom path with `--config`.
- **HTTP & HTTPS with Auto-TLS**: Native TLS support via `rustls`, including auto-generating self-signed X.509 certificates for localhost and your LAN IP.
- **Configurable CORS**: Customizable allowed origins, methods, and headers for web and WebSocket endpoints.
- **Omarchy Shell Integration**: Native bar widget (`KeyboardPanel` + `BarIconButton`) with live broadcast status, client count, and an interactive popout panel.
- **Enabled / Disabled Control**: Toggle audio sharing on and off directly from the Omarchy bar (right-click to toggle, or use the switch in the panel).
- **Ultra-Low Latency (~30–50ms)**: Streams 48kHz 16-bit stereo PCM with scheduled jitter buffers.
- **Device Identification**: Clients can customize their device name in the browser, stored in `localStorage` and visible on the host desktop.
- **Comprehensive Test Suite & Quality**: Strict >= 80% test coverage enforced across both Rust (`cargo-llvm-cov`) and TypeScript (`Vitest`), with zero clippy/ESLint warnings.

---

## Installation

### Via Cargo (Recommended)

```bash
# Clone the repository
git clone https://github.com/rodvieirasilva/soundthis.git
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

## Configuration (`config.toml`)

SoundThis automatically creates a default configuration file at `~/.config/soundthis/config.toml` on first run if it does not already exist.

```toml
# SoundThis Configuration File
# Location: ~/.config/soundthis/config.toml

# Server port (default: 9697)
port = 9697

# Bind address (default: "0.0.0.0" accepts local and LAN connections)
host = "0.0.0.0"

# Protocol: "http" or "https" (default: "http")
protocol = "http"

[tls]
# Path to TLS certificate and private key in PEM format (used when protocol is "https")
# If left unset or set to auto_generate = true, a self-signed certificate is auto-generated
# cert_path = "~/.config/soundthis/certs/cert.pem"
# key_path = "~/.config/soundthis/certs/key.pem"
auto_generate = true

[cors]
# Allowed origins for HTTP and WebSocket requests (default: ["*"])
allow_origins = ["*"]
allow_methods = ["GET", "POST", "OPTIONS"]
allow_headers = ["*"]

[audio]
# Audio capture settings for PipeWire / PulseAudio
sample_rate = 48000
channels = 2
chunk_size = 3840
latency_ms = 20
```

You can also specify a custom configuration file using the `--config` flag:
```bash
soundthis --config /path/to/my-config.toml serve
```

---

## Usage

### Web Client
Open the streaming page on any device connected to your network:

```
http://<YOUR_LAN_IP>:9697
# Or https://<YOUR_LAN_IP>:9697 if HTTPS is enabled
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
soundthis serve --https # Run server with HTTPS enabled
soundthis install     # Configure systemd daemon and Omarchy bar
soundthis uninstall   # Completely remove plugin and service
```

---

## Development, Testing & Quality

SoundThis uses `mise.toml` as the unified task runner for development, linting, testing, and coverage reports.

### Testing & Coverage (>= 80% Threshold)

```bash
mise run test            # Run all web and server unit/integration tests
mise run test:web        # Run frontend Vitest tests
mise run test:server     # Run Rust tests
mise run coverage        # Generate full coverage report for both Rust and Web
mise run coverage:rust   # Generate cargo-llvm-cov report (current: ~81.4% line coverage)
mise run coverage:web    # Generate Vitest v8 coverage report (current: ~90.9% line coverage)
```

### Linters & Formatting

```bash
mise run lint            # Run all linters (cargo clippy with -D warnings, rustfmt, ESLint)
mise run lint:rust       # Run cargo clippy & cargo fmt --check
mise run lint:web        # Run ESLint on frontend TypeScript
mise run fmt             # Automatically format Rust code
```

---

## Architecture (SOLID Principles)

- **`src/config.rs`**: Single responsibility for configuration parsing (TOML), TLS certificate resolution/generation, and CORS rules.
- **`src/audio/`**:
  - `source.rs`: `AudioSource` trait defining pluggable audio capture backends (`ParecAudioSource` for PipeWire/PulseAudio, `MockAudioSource` for testing).
  - `broadcaster.rs`: `AudioBroadcaster` fan-out channel with low-latency frame dropping for lagged clients.
- **`src/state.rs`**: Thread-safe `AppState` managing active connections and streaming toggles.
- **`src/server/`**:
  - `mod.rs`: Server bootstrapping and CORS configuration with support for both HTTP (`axum`) and HTTPS (`axum-server` with `tls-rustls`).
  - `http.rs`: HTTP endpoints and embedded asset delivery.
  - `ws.rs`: WebSocket streaming session handling and client identification.
- **`src/installer/`**: External templates (`systemd/soundthis.service.template`, `plugin/`) and systemd/Omarchy shell management.
- **`web/`**: Single-responsibility TypeScript modules:
  - `audio/player.ts`: `AudioStreamPlayer` managing AudioContext and scheduled audio buffer playback.
  - `audio/visualizer.ts`: `CanvasVisualizer` rendering responsive frequency spectrum bars.
  - `services/storage.ts`: Persistent device settings with validation.
  - `services/ws.ts`: Resilient WebSocket connection manager with auto-reconnect.
  - `ui/app.ts`: Application orchestrator and UI event coordinator.
