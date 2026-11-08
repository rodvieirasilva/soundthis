#!/bin/bash
set -euo pipefail

echo "=========================================="
echo "     Uninstalling SoundThis for Omarchy   "
echo "=========================================="

# 1. Stop and disable systemd service
echo "[1/4] Stopping and removing systemd user service..."
if systemctl --user is-active --quiet soundthis.service 2>/dev/null; then
    systemctl --user stop soundthis.service 2>/dev/null || true
fi
if systemctl --user is-enabled --quiet soundthis.service 2>/dev/null; then
    systemctl --user disable soundthis.service 2>/dev/null || true
fi
rm -f "$HOME/.config/systemd/user/soundthis.service"
systemctl --user daemon-reload 2>/dev/null || true

# 2. Disable and remove Omarchy shell plugin
echo "[2/4] Removing Omarchy shell plugin..."
if command -v omarchy >/dev/null 2>&1; then
    omarchy plugin disable soundthis 2>/dev/null || true
fi

PLUGIN_DIR="$HOME/.config/omarchy/plugins/soundthis"
if [[ -d "$PLUGIN_DIR" ]]; then
    rm -rf "$PLUGIN_DIR"
fi

if command -v omarchy-shell >/dev/null 2>&1; then
    omarchy-shell shell rescanPlugins 2>/dev/null || true
fi

# 3. Remove application binaries and files
echo "[3/4] Removing binaries and application data..."
rm -f "$HOME/.local/bin/soundthis"
rm -rf "$HOME/.local/share/soundthis"

# 4. Finish
echo "[4/4] Cleanup complete!"
echo ""
echo "=========================================="
echo "  ✓ SoundThis has been uninstalled."
echo "=========================================="
