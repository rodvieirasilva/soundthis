#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-${ROOT_DIR}/docs/screenshots/omarchy-panel.png}"

echo "📸 Capturing Omarchy desktop widget screenshot..."

# 1. Verify dependencies
for cmd in grim hyprctl jq qs pgrep; do
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

# 4. Calculate geometry based on focused monitor
FOCUSED=$(hyprctl monitors -j | jq -r '.[] | select(.focused)')
MON_X=$(echo "$FOCUSED" | jq -r '.x')
MON_Y=$(echo "$FOCUSED" | jq -r '.y')
MON_W=$(echo "$FOCUSED" | jq -r '.width')

X=$(( MON_X + MON_W - 386 ))
Y=$(( MON_Y + 22 ))
GEOM="${X},${Y} 380x385"

# 5. Open widget via quickshell IPC
qs ipc --pid "$PID" call soundthis open
sleep 0.6

# 6. Capture widget region
mkdir -p "$(dirname "$OUTPUT")"
grim -g "$GEOM" "$OUTPUT"

# 7. Close widget
qs ipc --pid "$PID" call soundthis close

echo "✅ Successfully captured Omarchy widget screenshot: $OUTPUT"
