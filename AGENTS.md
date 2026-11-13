# AGENTS.md — AI Agent & Contributor Guidelines for SoundThis

Welcome! This document provides essential architecture patterns, engineering standards, and instructions for AI agents and human contributors working on the **SoundThis** repository.

---

## 1. Project Overview & Mission

**SoundThis** is an ultra-low latency, real-time desktop audio streaming server built natively for **Omarchy Linux** in Rust and TypeScript/Vite.

- **Audio Capture**: Captures desktop playback directly from PipeWire / PulseAudio via `parec`.
- **Server**: Asynchronous Rust server built on Axum and Tokio with sub-3ms startup and ~1.6MB RAM footprint.
- **Embedded Web Client**: Modern single-file client (`TypeScript` + `Vite` + `AudioContext`), compiled and embedded directly into the binary via `include_str!`. Zero external runtime assets or internet access required.
- **Protocol**: HTTP/HTTPS with auto-generated self-signed TLS certificates (via `rcgen`) and raw PCM binary streaming over WebSockets.
- **Desktop Integration**: Native Omarchy shell bar widget (`plugin/Panel.qml`) and systemd user service (`soundthis.service`).

---

## 2. Repository Layout

```text
soundthis/
├── .github/
│   └── workflows/
│       ├── ci.yml                 # Continuous integration (lint, test, coverage, build)
│       └── release.yml            # Automated GitHub releases on v* git tags
├── assets/
│   └── favicon.ico                # Root-level server icon (served directly by Axum)
├── config/
│   └── config.toml.template       # External template for ~/.config/soundthis/config.toml
├── docs/
│   └── screenshots/               # UI and desktop screenshots for documentation
├── plugin/
│   ├── manifest.json              # Omarchy shell plugin manifest
│   └── Panel.qml                  # Omarchy desktop bar widget and popout panel
├── scripts/
│   └── capture-widget.sh          # Desktop widget screenshot automation via grim & quickshell
├── systemd/
│   └── soundthis.service.template # Systemd user service template with %BIN_PATH%
├── src/
│   ├── main.rs                    # Application entry point and CLI command dispatch
│   ├── lib.rs                     # Library exports for testing and modularity
│   ├── cli.rs                     # Clap CLI subcommands (status, toggle, enable, open, etc.)
│   ├── config.rs                  # Server configuration, TOML parser, TLS cert generator
│   ├── error.rs                   # SoundThisError enum definition
│   ├── state.rs                   # Shared thread-safe AppState (streaming toggle, client list)
│   ├── audio/
│   │   ├── mod.rs                 # Audio module exports
│   │   ├── source.rs              # AudioSource trait, ParecAudioSource, MockAudioSource
│   │   └── broadcaster.rs         # AudioBroadcaster broadcast channel with lag handling
│   ├── installer/
│   │   ├── mod.rs                 # Systemd & Omarchy plugin installer lifecycle
│   │   └── systemd.rs             # systemctl daemon-reload and service controls
│   ├── network/
│   │   ├── mod.rs                 # Network utilities
│   │   └── ip.rs                  # LAN IP detection for QR/client URLs
│   └── server/
│       ├── mod.rs                 # HTTP & HTTPS server runner, TLS config, CORS setup
│       ├── http.rs                # Axum HTTP routes (/health, /, /favicon.svg, /api/*)
│       └── ws.rs                  # WebSocket streaming and client identification
├── tests/
│   ├── cli_test.rs                # CLI command parsing and offline error handling tests
│   ├── config_test.rs             # TOML parsing, TLS generation, and template validation tests
│   ├── installer_test.rs          # Service/plugin installation lifecycle and idempotency tests
│   ├── http_test.rs               # Axum routes, CORS headers, and AppState tests
│   ├── package_test.rs            # Package metadata, staging layout, and lifecycle tests
│   ├── package/                   # Package E2E lifecycle test runner (zero mocks)
│   │   └── test-package-e2e.sh    # Real makepkg and pacman lifecycle verification
│   └── e2e_test.rs                # End-to-end tests (HTTP, HTTPS, WebSocket streaming, CLI)
├── web/
│   ├── index.html                 # Frontend HTML shell
│   ├── vite.config.ts             # Vite singlefile bundler + inline SVG plugin
│   ├── package.json               # Frontend dependencies and scripts
│   ├── e2e/
│   │   └── browser.test.ts        # Playwright E2E browser tests (sound, console, memory leaks)
│   ├── scripts/
│   │   └── capture-screenshots.mjs# Headless Playwright script for desktop & mobile screenshots
│   └── src/
│       ├── main.ts                # Frontend entrypoint
│       ├── style.css              # Tokyo Night themed UI styles
│       ├── assets/icons/          # External SVG icons (logo.svg, play.svg, pause.svg, volume.svg)
│       ├── audio/
│       │   ├── player.ts          # AudioStreamPlayer (Web Audio API jitter buffer scheduler)
│       │   └── visualizer.ts      # CanvasVisualizer (responsive frequency bars)
│       ├── services/
│       │   ├── storage.ts         # Persistent client settings with validation
│       │   └── ws.ts              # Resilient WebSocket connection manager
│       └── ui/
│           └── app.ts             # UI orchestrator and DOM event binding
├── Cargo.toml                     # Rust package configuration and dependencies
├── mise.toml                      # Unified task runner configuration
└── README.md                      # End-user documentation and installation guides
```

---

## 3. Core Architecture Rules & Constraints

### A. Single Responsibility & Modular Design (SOLID)
- Keep files focused and concise. **Never create monolithic multi-thousand-line files.**
- If a file grows beyond 300–400 lines, evaluate splitting it into focused modules.
- Integration and E2E tests MUST be organized into dedicated, single-responsibility files in `tests/` (`cli_test.rs`, `config_test.rs`, `e2e_test.rs`, `http_test.rs`, `installer_test.rs`).

### B. No Long String Constants — Always Use External Templates
- **NEVER** embed large multi-line string constants in Rust code.
- Static assets, configuration files, systemd units, and SVGs must reside in external files:
  - Configuration defaults: `config/config.toml.template`
  - Systemd unit: `systemd/soundthis.service.template`
  - Plugin metadata: `plugin/manifest.json` and `plugin/Panel.qml`
  - Icons: `web/src/assets/icons/*.svg`
- Embedded assets are included at compile time via `include_str!` and accompanied by validation functions (e.g., `ServerConfig::validate_template()`, `Installer::validate_templates()`).

### C. Single Source of Truth for SVGs & Favicons
- Never duplicate SVG code across HTML, CSS, or TypeScript files.
- Store icons in `web/src/assets/icons/` and use Vite's `inlineSvgPlugin` to inject `<svg-icon src="..." />` tags into inline `<svg>` elements at build time. Keep this plugin concise and focused only on SVG tag transformation.
- The web player logo and the `/favicon.svg` route both source directly from `web/src/assets/icons/logo.svg`.
- Static binary assets intended for root HTTP serving (e.g. `assets/favicon.ico`) must reside in `assets/`, never in `web/` root. This prevents Vite's Rollup bundler from extracting separate hashed asset files (`favicon-[hash].ico`).

### D. Zero External Web Runtime Dependencies
- The web client must build into a single self-contained HTML file (`web/dist/index.html`) using `vite-plugin-singlefile`.
- No external CDNs, fonts, or external images at runtime. Everything is bundled and served directly by the single Rust binary.

### E. Protocol & Streaming Efficiency
- **Initial Connection State**: Browsers connect to the WebSocket endpoint initially in a paused state (`?listen=0`).
- **Zero Audio While Paused**: WebSocket worker loops must discard broadcast audio chunks when a listener is paused or when broadcasting is disabled. Old audio chunks are not buffered during pause, ensuring instant, lag-free playback upon resume.
- **Dynamic Telemetry**: The `/api/status` endpoint exposes the server version, protocol, streaming state, and listener states (`listening: true/false`).

---

## 4. Quality Gates & Testing Standards

All pull requests and code modifications must pass the following strict quality gates:

| Metric | Target | Tool / Command |
|---|---|---|
| **Rust Test Coverage** | **>= 80% Lines & Functions** | `cargo llvm-cov --all-targets --summary-only` |
| **Rust Clippy** | **0 Warnings (Strict `-D warnings`)** | `cargo clippy --all-targets -- -D warnings` |
| **Rust Formatting** | **100% Compliant** | `cargo fmt --check` |
| **Web Test Coverage** | **>= 80% Lines & Functions** | `cd web && npm run test:coverage` |
| **Web ESLint** | **0 Warnings** | `cd web && npm run lint` |
| **Playwright Browser E2E** | **100% Passing (0 console errors, 0 404s, memory leak free)** | `mise run test:e2e` |

### Running Quality Checks & Lifecycle Tasks via Mise
```bash
# Run all unit and integration tests (Rust + Vitest)
mise run test

# Run browser E2E tests (audio streaming, console errors, memory stability)
mise run test:e2e

# Check all linters (Rust + TypeScript)
mise run lint

# Check test coverage for both suites
mise run coverage

# Format code
mise run fmt

# Recompile, update cargo binary, install plugin, and reload Omarchy shell
mise run reinstall

# Refresh documentation screenshots (web desktop/mobile + Omarchy panel)
mise run screenshots
```

---

## 5. Git Commit & Authorship Standards

- **Commit Message Format**:
  Follow Conventional Commits:
  - `feat: ...` for new features
  - `fix: ...` for bug fixes
  - `refactor: ...` for architectural improvements without behavior changes
  - `test: ...` for test suite additions and reorganizations
  - `docs: ...` for documentation updates
  - `ci: ...` for GitHub Actions and pipeline changes
  - `chore: ...` for dependency updates and maintenance

---

## 6. Pre-Commit Checklist for AI Agents

Before declaring any task or ticket complete, execute this checklist:
1. `mise run lint` (or `cargo clippy --all-targets -- -D warnings && cargo fmt --check && cd web && npm run lint`) passes with **0 warnings**.
2. `mise run test` passes **100% of Rust and Vitest tests**.
3. `mise run test:e2e` passes **100% of Playwright browser tests** (verifying zero console warnings/errors and zero 404s).
4. `mise run coverage` verifies both Rust and Web suites remain **>= 80%**.
5. Any new templates, icons, or configurations are created as **external files**, not embedded string constants.
6. If QML or web UI styling changed, update screenshots via `mise run screenshots`.
