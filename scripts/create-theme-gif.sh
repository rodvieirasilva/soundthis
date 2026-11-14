#!/usr/bin/env bash
# Generate an animated GIF demonstrating live Omarchy theme synchronization across
# the Web Client, Native Desktop App, and Omarchy Shell Bar Widget.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-${ROOT_DIR}/docs/screenshots/live-theme-sync.gif}"
TMP_DIR=$(mktemp -d /tmp/soundthis_gif_XXXXXX)

echo "🎬 Starting SoundThis live theme synchronization GIF generator..."

# 1. Verify dependencies
for cmd in grim hyprctl jq qs magick ffmpeg node; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "⚠️ Required tool '$cmd' not found. Please ensure it is installed and in PATH." >&2
    exit 1
  fi
done

# 2. Locate or build soundthis binary
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
  echo "⚙️ Building soundthis in debug mode..."
  cargo build --manifest-path "${ROOT_DIR}/Cargo.toml"
  BIN="${ROOT_DIR}/target/debug/soundthis"
fi

# 3. Quickshell, Plugin sync, and Monitor detection
PLUGIN_DEST="${HOME}/.config/omarchy/plugins/soundthis"
if [ -d "$PLUGIN_DEST" ]; then
  if ! cmp -s "${ROOT_DIR}/plugin/Panel.qml" "${PLUGIN_DEST}/Panel.qml"; then
    cp -f "${ROOT_DIR}/plugin/Panel.qml" "${PLUGIN_DEST}/Panel.qml"
    omarchy restart shell 2>/dev/null || true
    sleep 1.2
  fi
fi

PID=$(pgrep -x quickshell | head -n 1 || true)
if [ -z "$PID" ]; then
  echo "⚠️ Quickshell is not running. Please start Omarchy shell first." >&2
  exit 1
fi

MONITOR=$(hyprctl monitors -j | jq -r '(.[] | select(.id == 0)) // .[0]')
MON_X=$(echo "$MONITOR" | jq -r '.x')
MON_Y=$(echo "$MONITOR" | jq -r '.y')
MON_W=$(echo "$MONITOR" | jq -r '.width')

CARD_X=$(( MON_X + MON_W - 390 ))
CARD_Y=$(( MON_Y + 31 ))
CARD_GEOM="${CARD_X},${CARD_Y} 360x324"

# 4. Ensure server is active
SERVER_SPAWNED=0
if ! curl -fsS --max-time 1 http://127.0.0.1:9697/api/status >/dev/null 2>&1; then
  echo "ℹ️ Starting background SoundThis server on port 9697..."
  "$BIN" serve --port 9697 &
  SERVER_PID=$!
  SERVER_SPAWNED=1
  for _ in $(seq 1 30); do
    if curl -fsS --max-time 1 http://127.0.0.1:9697/api/status >/dev/null 2>&1; then
      break
    fi
    sleep 0.1
  done
fi

# 5. Save original theme and set trap
ORIGINAL_THEME=$(omarchy theme current)
echo "📌 Saved original desktop theme: $ORIGINAL_THEME"

cleanup() {
  echo "🧹 Cleaning up..."
  if [ -n "${ORIGINAL_THEME:-}" ]; then
    echo "🔄 Restoring original theme: $ORIGINAL_THEME"
    omarchy theme set "$ORIGINAL_THEME" >/dev/null 2>&1 || true
  fi
  if [ "$SERVER_SPAWNED" -eq 1 ] && [ -n "${SERVER_PID:-}" ]; then
    kill "$SERVER_PID" 2>/dev/null || true
  fi
  pkill -f "soundthis app" 2>/dev/null || true
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

# Silence crash balloons and dismiss notifications to keep screenshots clean
omarchy crash mute soundthis on 2>/dev/null || true
omarchy notification dismiss "crashed" 2>/dev/null || true
omarchy notification dismiss "soundthis" 2>/dev/null || true

# 6. Capture theme frame function
capture_theme_frame() {
  local theme="$1"
  local index="$2"
  echo "🎨 Applying theme: $theme..."
  omarchy theme set "$theme"
  sleep 1.6

  local toml_path="$HOME/.local/state/omarchy/current/theme/colors.toml"
  local bg=$(grep -E '^\s*background\s*=' "$toml_path" | head -n1 | cut -d'"' -f2 || echo "#1a1b26")
  local fg=$(grep -E '^\s*foreground\s*=' "$toml_path" | head -n1 | cut -d'"' -f2 || echo "#c0caf5")
  local accent=$(grep -E '^\s*accent\s*=' "$toml_path" | head -n1 | cut -d'"' -f2 || echo "#7aa2f7")
  local dark_bg=$(grep -E '^\s*dark_background\s*=' "$toml_path" | head -n1 | cut -d'"' -f2 || echo "#16161e")

  # 6A. Capture Web Client using external Playwright runner
  echo "   [1/3] Capturing Web Client..."
  node "${ROOT_DIR}/web/scripts/capture-panel.mjs" 9697 "${TMP_DIR}/web_panel.png" 400 600

  # 6B. Capture Omarchy Bar Widget
  echo "   [2/3] Capturing Omarchy Bar Widget..."
  qs ipc --pid "$PID" call soundthis open
  sleep 0.6
  grim -g "$CARD_GEOM" "${TMP_DIR}/widget_raw.png"
  qs ipc --pid "$PID" call soundthis close
  magick -size 400x600 xc:"$bg" "${TMP_DIR}/widget_raw.png" -gravity center -composite "${TMP_DIR}/widget_panel.png"

  # 6C. Capture Native Desktop App
  echo "   [3/3] Capturing Desktop App..."
  omarchy notification dismiss "crashed" 2>/dev/null || true
  "$BIN" app &
  local app_pid=$!
  local app_geom=""
  for _ in $(seq 1 40); do
    app_geom=$(hyprctl clients -j 2>/dev/null | jq -r '.[] | select(.initialClass == "com.omarchy.soundthis" or .class == "com.omarchy.soundthis") | "\(.at[0]),\(.at[1]) \(.size[0])x\(.size[1])"' | head -n1 || true)
    [ -n "$app_geom" ] && break
    sleep 0.1
  done
  sleep 0.5
  grim -g "$app_geom" "${TMP_DIR}/app_raw.png"
  kill "$app_pid" 2>/dev/null || true
  wait "$app_pid" 2>/dev/null || true
  magick -size 400x600 xc:"$bg" \( "${TMP_DIR}/app_raw.png" -resize 400x600 \) -gravity center -composite "${TMP_DIR}/app_panel.png"

  # 6D. Add header badges above each column
  label_panel() {
    local img="$1"
    local label="$2"
    local out="$3"
    magick "$img" \
      -background "$dark_bg" -fill "$fg" -font "Liberation-Sans-Bold" -pointsize 14 \
      -gravity north -splice 0x32 \
      -annotate +0+8 "  $label  " \
      "$out"
  }

  label_panel "${TMP_DIR}/web_panel.png" "Web Client" "${TMP_DIR}/lbl_web.png"
  label_panel "${TMP_DIR}/app_panel.png" "Desktop App" "${TMP_DIR}/lbl_app.png"
  label_panel "${TMP_DIR}/widget_panel.png" "Omarchy Bar Widget" "${TMP_DIR}/lbl_widget.png"

  # 6E. Combine panels side by side
  magick "${TMP_DIR}/lbl_web.png" "${TMP_DIR}/lbl_app.png" "${TMP_DIR}/lbl_widget.png" +append "${TMP_DIR}/row.png"

  # 6F. Add top banner with theme title and accent indicator
  magick "${TMP_DIR}/row.png" \
    -background "$dark_bg" -fill "$accent" -font "Liberation-Sans-Bold" -pointsize 18 \
    -gravity north -splice 0x54 \
    -annotate +0+16 "● Theme: ${theme}" \
    "${TMP_DIR}/frame_${index}.png"

  echo "   ✅ Frame ${index} generated (${theme})"
}

# 7. Cycle through distinct Omarchy themes (Dark, Light, Retro, Monochrome)
capture_theme_frame "Tokyo Night" "01"
capture_theme_frame "Catppuccin Latte" "02"
capture_theme_frame "Gruvbox" "03"
capture_theme_frame "White" "04"

# 8. Encode optimized animated GIF
echo "🎞️ Assembling animated GIF with ffmpeg..."
mkdir -p "$(dirname "$OUTPUT")"
ffmpeg -y -framerate 0.55 -i "${TMP_DIR}/frame_%02d.png" \
  -vf "split[s0][s1];[s0]palettegen=max_colors=128:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3" \
  "$OUTPUT"

GIF_SIZE=$(du -h "$OUTPUT" | cut -f1)
echo "🎉 Live theme sync GIF successfully generated!"
echo "   Output: $OUTPUT"
echo "   Size:   $GIF_SIZE"
