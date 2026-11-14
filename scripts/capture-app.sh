#!/usr/bin/env bash
# Capture live screenshot of the native SoundThis GTK4 / Libadwaita desktop application
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-${ROOT_DIR}/docs/screenshots/desktop-app.png}"

echo "📸 Capturing SoundThis native desktop application screenshot..."

# 1. Verify dependencies
for cmd in grim hyprctl jq pgrep; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "⚠️ Required tool '$cmd' not found. Cannot capture desktop window." >&2
    exit 1
  fi
done

# 2. Locate binary
BIN_CANDIDATES=(
  "${ROOT_DIR}/target/release/soundthis"
  "${ROOT_DIR}/target/debug/soundthis"
  "soundthis"
)
BIN=""
for b in "${BIN_CANDIDATES[@]}"; do
  if [ -x "$b" ] || command -v "$b" >/dev/null 2>&1; then
    BIN="$b"
    break
  fi
done

if [ -z "$BIN" ]; then
  echo "⚠️ soundthis executable not found. Build with 'cargo build --release' first." >&2
  exit 1
fi

# 3. Launch application in the background
"$BIN" &
APP_PID=$!

cleanup() {
  if kill -0 "$APP_PID" 2>/dev/null; then
    kill "$APP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# 4. Wait for the GTK4 window to appear on Hyprland
GEOM=""
for _ in $(seq 1 40); do
  GEOM=$(hyprctl clients -j 2>/dev/null | jq -r '.[] | select(.initialClass == "com.omarchy.soundthis" or .class == "com.omarchy.soundthis") | "\(.at[0]),\(.at[1]) \(.size[0])x\(.size[1])"' | head -n1 || true)
  if [ -n "$GEOM" ]; then
    break
  fi
  sleep 0.1
done

if [ -z "$GEOM" ]; then
  echo "⚠️ SoundThis GTK window did not appear within timeout." >&2
  exit 1
fi

# 5. Brief stabilization pause
sleep 0.5

# 6. Capture screenshot
mkdir -p "$(dirname "$OUTPUT")"
grim -g "$GEOM" "$OUTPUT"

echo "✅ Successfully captured desktop app screenshot: $OUTPUT"
