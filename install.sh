#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=========================================="
echo "      Installing SoundThis for Omarchy    "
echo "=========================================="

# 1. Check prerequisites
echo "[1/5] Checking prerequisites..."
command -v python3 >/dev/null || { echo "Error: python3 is required." >&2; exit 1; }
command -v parec >/dev/null || { echo "Error: parec (Pipewire/PulseAudio) is required." >&2; exit 1; }
command -v omarchy >/dev/null || { echo "Error: omarchy CLI is required." >&2; exit 1; }

# 2. Install application files
echo "[2/5] Installing core server and web client..."
mkdir -p "$HOME/.local/bin"
mkdir -p "$HOME/.local/share/soundthis/web"

cp "$SCRIPT_DIR/soundthis" "$HOME/.local/bin/soundthis"
chmod +x "$HOME/.local/bin/soundthis"

cp "$SCRIPT_DIR/server.py" "$HOME/.local/share/soundthis/server.py"
chmod +x "$HOME/.local/share/soundthis/server.py"

cp "$SCRIPT_DIR/uninstall.sh" "$HOME/.local/share/soundthis/uninstall.sh"
chmod +x "$HOME/.local/share/soundthis/uninstall.sh"

cp "$SCRIPT_DIR/web/index.html" "$HOME/.local/share/soundthis/web/index.html"

# 3. Validate and install Omarchy shell plugin
echo "[3/5] Validating and installing Omarchy plugin..."
omarchy plugin validate "$SCRIPT_DIR/plugin"

PLUGIN_DEST="$HOME/.config/omarchy/plugins/soundthis"
mkdir -p "$PLUGIN_DEST"
# Clean previous files if any (ensure no symlinks)
rm -rf "${PLUGIN_DEST:?}"/*
cp -r "$SCRIPT_DIR/plugin/"* "$PLUGIN_DEST/"

# Rescan shell plugins
omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true

# Enable plugin in Omarchy bar
echo "[4/5] Enabling SoundThis in Omarchy bar..."
if omarchy plugin list --json | jq -e '.[] | select(.id == "soundthis" and .enabled == true)' >/dev/null 2>&1; then
    echo "  Plugin 'soundthis' is already enabled."
else
    omarchy plugin enable soundthis --section right || true
fi

# 4. Install systemd user service
echo "[5/5] Configuring systemd user service..."
SYSTEMD_USER_DIR="$HOME/.config/systemd/user"
mkdir -p "$SYSTEMD_USER_DIR"
cp "$SCRIPT_DIR/systemd/soundthis.service" "$SYSTEMD_USER_DIR/soundthis.service"

systemctl --user daemon-reload
systemctl --user enable --now soundthis.service

echo ""
echo "=========================================="
echo "  ✓ SoundThis installed successfully!"
echo "=========================================="
echo ""
echo "Server URL: http://localhost:9697"
lan_ip=$(python3 -c "import socket; s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.connect(('8.8.8.8', 80)); print(s.getsockname()[0]); s.close()" 2>/dev/null || echo "127.0.0.1")
echo "LAN URL:    http://$lan_ip:9697"
echo ""
echo "Try running:"
echo "  soundthis status    - View live streaming status"
echo "  soundthis toggle    - Toggle audio sharing on/off"
echo "  soundthis open      - Open web player in browser"
echo ""
