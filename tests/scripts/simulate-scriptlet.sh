#!/usr/bin/env bash
# Scriptlet lifecycle simulation test runner for soundthis.install
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"

TEMP_DIR="$(mktemp -d -t soundthis_scriptlet_test.XXXXXX)"
trap 'rm -rf "$TEMP_DIR"' EXIT

BIN_DIR="$TEMP_DIR/bin"
mkdir -p "$BIN_DIR"

SHELL_JSON="$TEMP_DIR/shell.json"
echo '{"bar":{"layout":{"right":[{"id":"omarchy.tray"}]}}}' > "$SHELL_JSON"

# Link / copy mock binaries
cp "$SCRIPT_DIR/mock-omarchy-shell.sh" "$BIN_DIR/omarchy-shell"
cp "$SCRIPT_DIR/mock-sudo.sh" "$BIN_DIR/sudo"
chmod +x "$BIN_DIR/omarchy-shell" "$BIN_DIR/sudo"

RUNTIME_DIR="$TEMP_DIR/run"
mkdir -p "$RUNTIME_DIR"
touch "$RUNTIME_DIR/wayland-1"

export PATH="$BIN_DIR:$PATH"
export XDG_RUNTIME_DIR="$RUNTIME_DIR"
export MOCK_SHELL_JSON="$SHELL_JSON"

# Source the install scriptlet
# shellcheck source=/dev/null
source "$REPO_ROOT/pkg/soundthis.install"

_get_target_users() {
  echo "1000"
}

# 1. Execute post_install
post_install

# Verify insertion into shell.json
jq -e 'any(.bar.layout.right[]; .id == "soundthis" or . == "soundthis")' "$SHELL_JSON" >/dev/null

# 2. Execute post_install again for idempotency
post_install
count=$(jq '[.bar.layout.right[] | select(.id == "soundthis" or . == "soundthis")] | length' "$SHELL_JSON")
if [ "$count" -ne 1 ]; then
  echo "Expected exactly 1 soundthis entry, found $count" >&2
  exit 1
fi

# 3. Execute pre_remove
pre_remove

# Verify removal from shell.json
if jq -e 'any(.bar.layout.right[]; .id == "soundthis" or . == "soundthis")' "$SHELL_JSON" >/dev/null 2>&1; then
  echo "soundthis was not removed from shell.json" >&2
  exit 1
fi

# 4. Execute post_remove
post_remove

echo "Scriptlet lifecycle simulation passed successfully."
