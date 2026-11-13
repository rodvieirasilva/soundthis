#!/usr/bin/env bash
# Mock omarchy-shell for testing scriptlet widget registration
set -euo pipefail

TARGET_JSON="${MOCK_SHELL_JSON:-${1:-}}"

case "$*" in
  *enablePlugin*soundthis*)
    if [[ -n "$TARGET_JSON" && -f "$TARGET_JSON" ]]; then
      jq 'if (.bar.layout.right | any(.id == "soundthis" or . == "soundthis")) then . else .bar.layout.right = [{"id": "soundthis"}] + .bar.layout.right end' "$TARGET_JSON" > "${TARGET_JSON}.tmp" && mv "${TARGET_JSON}.tmp" "$TARGET_JSON"
    fi
    echo "ok"
    ;;
  *setPluginEnabled*soundthis*false*)
    if [[ -n "$TARGET_JSON" && -f "$TARGET_JSON" ]]; then
      jq '.bar.layout.right = [.bar.layout.right[] | select(.id != "soundthis" and . != "soundthis")]' "$TARGET_JSON" > "${TARGET_JSON}.tmp" && mv "${TARGET_JSON}.tmp" "$TARGET_JSON"
    fi
    echo "ok"
    ;;
  *)
    echo "ok"
    ;;
esac
