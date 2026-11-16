# Stage 1: Build singlefile Vite web frontend
FROM node:22-alpine AS web-builder
WORKDIR /app/web
COPY web/package*.json ./
RUN npm ci --prefer-offline || npm install
COPY web/ ./
RUN npm run build

# Stage 2: Build Rust single binary (headless, no-default-features)
FROM rust:alpine AS rust-builder
RUN apk add --no-cache musl-dev build-base perl
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Pre-copy web dist from stage 1 so include_str! succeeds
COPY --from=web-builder /app/web/dist ./web/dist
COPY src ./src
COPY config ./config
COPY systemd ./systemd
COPY plugin ./plugin
COPY assets ./assets
COPY pkg ./pkg
COPY web/src/assets ./web/src/assets
RUN cargo build --release --no-default-features

# Stage 3: Alpine Linux runtime container
FROM alpine:latest
LABEL maintainer="Rodrigo Vieira <rodvieirasilva@gmail.com>" \
      description="SoundThis - Ultra-low latency desktop audio streaming server for Omarchy"

RUN apk add --no-cache \
    pulseaudio-utils \
    ca-certificates

# Create non-root user and required runtime directories
RUN (getent group audio >/dev/null || addgroup -g 29 audio || addgroup audio) && \
    adduser -D -u 1000 -s /bin/sh soundthis && \
    addgroup soundthis audio && \
    mkdir -p /home/soundthis/.config/soundthis /home/soundthis/.config/pulse && \
    chown -R soundthis:soundthis /home/soundthis

# Copy compiled soundthis binary
COPY --from=rust-builder /app/target/release/soundthis /usr/bin/soundthis

# Expose default HTTP/WebSocket port
EXPOSE 9697

# Connect to shared host PipeWire / PulseAudio socket
ENV PULSE_SERVER=unix:/tmp/pulse/native \
    HOME=/home/soundthis

USER soundthis
WORKDIR /home/soundthis

# Serve audio over HTTP/WebSocket
ENTRYPOINT ["/usr/bin/soundthis"]
CMD ["serve"]
