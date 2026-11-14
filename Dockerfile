# Stage 1: Build singlefile Vite web frontend
FROM node:22-alpine AS web-builder
WORKDIR /app/web
COPY web/package*.json ./
RUN npm ci --prefer-offline || npm install
COPY web/ ./
RUN npm run build

# Stage 2: Build Rust single binary (headless, no-default-features)
FROM rust:1-slim-bookworm AS rust-builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Pre-copy web dist from stage 1 so include_str! succeeds
COPY --from=web-builder /app/web/dist ./web/dist
COPY src ./src
COPY config ./config
COPY systemd ./systemd
COPY plugin ./plugin
COPY assets ./assets
RUN cargo build --release --no-default-features

# Stage 3: Arch Linux runtime container (Native to Omarchy)
FROM archlinux:base
LABEL maintainer="Rodrigo Vieira <rodvieirasilva@gmail.com>" \
      description="SoundThis - Ultra-low latency desktop audio streaming server for Omarchy"

RUN pacman -Syu --noconfirm --needed \
    libpulse \
    ca-certificates \
    && rm -rf /var/cache/pacman/pkg/* /var/lib/pacman/sync/*

# Copy compiled soundthis binary
COPY --from=rust-builder /app/target/release/soundthis /usr/bin/soundthis

# Expose default HTTP/WebSocket port
EXPOSE 9697

# Connect to shared host PipeWire / PulseAudio socket
ENV PULSE_SERVER=unix:/tmp/pulse/native

# Serve audio over HTTP/WebSocket
ENTRYPOINT ["/usr/bin/soundthis", "serve"]
