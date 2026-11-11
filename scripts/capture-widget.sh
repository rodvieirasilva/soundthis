#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-${ROOT_DIR}/docs/screenshots/omarchy-panel.png}"

echo "📸 Capturing Omarchy desktop widget screenshot..."

# 1. Verify dependencies
for cmd in grim hyprctl jq qs pgrep magick; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "⚠️ Required tool '$cmd' not found. Cannot capture widget." >&2
    exit 1
  fi
done

# 2. Find Quickshell process
PID=$(pgrep -x quickshell | head -n 1 || true)
if [ -z "$PID" ]; then
  echo "⚠️ Quickshell is not running. Please start Omarchy shell first." >&2
  exit 1
fi

# 3. Ensure soundthis server is running for live preview
if ! curl -fsS --max-time 1 http://127.0.0.1:9697/api/status >/dev/null 2>&1; then
  echo "ℹ️ SoundThis service is not active, starting temporarily..."
  systemctl --user start soundthis 2>/dev/null || true
  sleep 1
fi

# 4. Calculate exact panel geometry based on focused monitor
FOCUSED=$(hyprctl monitors -j | jq -r '.[] | select(.focused)')
MON_X=$(echo "$FOCUSED" | jq -r '.x')
MON_Y=$(echo "$FOCUSED" | jq -r '.y')
MON_W=$(echo "$FOCUSED" | jq -r '.width')

# The Omarchy SoundThis panel card has width=360, height=330, anchored 30px from right margin
CARD_X=$(( MON_X + MON_W - 390 ))
CARD_Y=$(( MON_Y + 31 ))
CARD_W=360
CARD_H=330
GEOM="${CARD_X},${CARD_Y} ${CARD_W}x${CARD_H}"

# 5. Open widget via quickshell IPC
qs ipc --pid "$PID" call soundthis open
sleep 0.6

# 6. Capture widget card cleanly
TEMP_CARD=$(mktemp /tmp/soundthis_card_XXXXXX.png)
grim -g "$GEOM" "$TEMP_CARD"

# 7. Close widget
qs ipc --pid "$PID" call soundthis close

# 8. Frame card centered on a 680x600 canvas (#1a1b26) matching web-client.png
mkdir -p "$(dirname "$OUTPUT")"
magick -size 680x600 xc:"#1a1b26" "$TEMP_CARD" -gravity center -composite "$OUTPUT"
rm -f "$TEMP_CARD"

echo "✅ Successfully captured Omarchy widget screenshot: $OUTPUT"
