#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Testing plugin manifest and schema validation..."
omarchy plugin validate "$SCRIPT_DIR/plugin"

echo "Checking manifest fields..."
jq -e '.id == "soundthis"' "$SCRIPT_DIR/plugin/manifest.json" >/dev/null
jq -e '.schemaVersion == 1' "$SCRIPT_DIR/plugin/manifest.json" >/dev/null
jq -e '.kinds == ["bar-widget"]' "$SCRIPT_DIR/plugin/manifest.json" >/dev/null
jq -e '.entryPoints.barWidget == "Panel.qml"' "$SCRIPT_DIR/plugin/manifest.json" >/dev/null

echo "Plugin validation test passed successfully!"
