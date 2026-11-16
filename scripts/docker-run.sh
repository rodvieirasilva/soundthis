#!/usr/bin/env bash
set -euo pipefail

# Locate host PipeWire / PulseAudio native socket
PULSE_SOCKET="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/pulse/native"

if [[ ! -S "$PULSE_SOCKET" ]]; then
  echo "⚠️ Warning: Host audio socket not found at $PULSE_SOCKET"
  echo "Make sure PipeWire or PulseAudio is running on your host machine."
fi

# Detect docker or podman
if command -v podman >/dev/null 2>&1; then
  CONTAINER_ENGINE="podman"
elif command -v docker >/dev/null 2>&1; then
  if docker info >/dev/null 2>&1; then
    CONTAINER_ENGINE="docker"
  elif sudo -n docker info >/dev/null 2>&1; then
    CONTAINER_ENGINE="sudo docker"
  else
    CONTAINER_ENGINE="docker"
  fi
else
  echo "❌ Error: Neither docker nor podman was found in PATH." >&2
  exit 1
fi

IMAGE_NAME="soundthis:latest"

echo "🎵 Starting SoundThis in $CONTAINER_ENGINE container..."
echo "🔊 Sharing host audio socket: $PULSE_SOCKET"

# Ensure image exists or build it
if ! $CONTAINER_ENGINE image inspect "$IMAGE_NAME" >/dev/null 2>&1; then
  echo "📦 Building $IMAGE_NAME image..."
  $CONTAINER_ENGINE build -t "$IMAGE_NAME" .
fi

PULSE_COOKIE_ARG=()
if [[ -f "${HOME}/.config/pulse/cookie" ]]; then
  PULSE_COOKIE_ARG=(-v "${HOME}/.config/pulse/cookie:/home/soundthis/.config/pulse/cookie:ro")
fi

exec $CONTAINER_ENGINE run --rm -it \
  --name soundthis \
  --network host \
  -e PULSE_SERVER=unix:/tmp/pulse/native \
  -v "$PULSE_SOCKET:/tmp/pulse/native:ro" \
  ${PULSE_COOKIE_ARG[@]+"${PULSE_COOKIE_ARG[@]}"} \
  "$IMAGE_NAME" "$@"
