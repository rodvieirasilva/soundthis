#!/usr/bin/env bash
# End-to-End Package Installation & Lifecycle Verification for SoundThis
# Supports:
#   --docker     : Run the full pacman install/uninstall lifecycle in an Arch Linux container
#   --container  : Executed inside an Arch Linux container (automated by --docker or CI)
#   (default)    : Local sandbox test validating package build, file layout, and scriptlet lifecycle

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

info() { echo -e "${BLUE}::${NC} $*"; }
success() { echo -e "${GREEN}✅${NC} $*"; }
warn() { echo -e "${YELLOW}⚠️${NC} $*"; }
fail() { echo -e "${RED}❌${NC} $*" >&2; exit 1; }

# Case 1: Docker mode (invoked from host to run in Arch container)
if [[ "${1:-}" == "--docker" ]]; then
  info "Launching full package lifecycle E2E test in Arch Linux container..."
  docker run --rm \
    -v "$REPO_ROOT:/workspace" \
    -w /workspace \
    archlinux:base-devel \
    bash -c "/workspace/scripts/test-package-e2e.sh --container"
  exit $?
fi

# Case 2: Inside container mode (running as root in Arch Linux)
if [[ "${1:-}" == "--container" ]]; then
  info "Running inside Arch Linux container..."

  # Install required package dependencies
  pacman -Syu --noconfirm --needed jq

  # Setup unprivileged build user for makepkg
  if ! id -u builder >/dev/null 2>&1; then
    useradd -m -s /bin/bash builder
    echo "builder ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers
  fi

  BUILD_DIR="/tmp/soundthis_pkg_build"
  rm -rf "$BUILD_DIR"
  mkdir -p "$BUILD_DIR"
  cp -r "$REPO_ROOT"/pkg/* "$BUILD_DIR"/
  chown -R builder:builder "$BUILD_DIR"

  # Setup a mock Omarchy desktop user session
  MOCK_USER_HOME="/home/builder"
  MOCK_CONFIG_DIR="$MOCK_USER_HOME/.config/omarchy"
  mkdir -p "$MOCK_CONFIG_DIR"
  echo '{"bar":{"layout":{"right":[{"id":"omarchy.tray"}]}}}' > "$MOCK_CONFIG_DIR/shell.json"
  chown -R builder:builder "$MOCK_USER_HOME"

  # Build the package as user builder
  info "Building package with makepkg..."
  sudo -u builder bash -c "cd '$BUILD_DIR' && makepkg -fd --nodeps"

  PKG_FILE=$(find "$BUILD_DIR" -maxdepth 1 -name "soundthis-*.pkg.tar.zst" | head -n1)
  [[ -f "$PKG_FILE" ]] || fail "Package .pkg.tar.zst was not generated"
  success "Package generated: $(basename "$PKG_FILE")"

  # 1. Test Package Installation
  info "Installing package with pacman -U..."
  SUDO_USER=builder pacman -U --nodeps --noconfirm "$PKG_FILE"

  info "Verifying installed files..."
  [[ -x /usr/bin/soundthis ]] || fail "/usr/bin/soundthis is missing or not executable"
  [[ -f /usr/lib/systemd/user/soundthis.service ]] || fail "/usr/lib/systemd/user/soundthis.service is missing"
  [[ -f /usr/lib/systemd/user-preset/90-soundthis.preset ]] || fail "Systemd user preset is missing"
  [[ -f /usr/share/applications/soundthis.desktop ]] || fail "Desktop file is missing"
  [[ -f /usr/share/icons/hicolor/scalable/apps/soundthis.svg ]] || fail "Scalable SVG icon is missing"
  [[ -f /usr/share/omarchy/shell/plugins/soundthis/manifest.json ]] || fail "Plugin manifest.json is missing"
  [[ -f /usr/share/omarchy/shell/plugins/soundthis/Panel.qml ]] || fail "Plugin Panel.qml is missing"
  success "All package files verified in /usr"

  # Test binary output
  info "Testing binary execution..."
  /usr/bin/soundthis --version
  success "soundthis binary runs and reports version"

  # Verify systemd preset content
  grep -q "enable soundthis.service" /usr/lib/systemd/user-preset/90-soundthis.preset || fail "Preset does not enable soundthis.service"
  success "Systemd user preset correctly enables soundthis.service"

  # 2. Test Package Uninstallation
  info "Uninstalling package with pacman -Rns..."
  SUDO_USER=builder pacman -Rns --nodeps --noconfirm soundthis

  info "Verifying package removal..."
  [[ ! -f /usr/bin/soundthis ]] || fail "/usr/bin/soundthis was not removed"
  [[ ! -f /usr/lib/systemd/user/soundthis.service ]] || fail "Systemd service was not removed"
  [[ ! -f /usr/lib/systemd/user-preset/90-soundthis.preset ]] || fail "Preset was not removed"
  [[ ! -f /usr/share/applications/soundthis.desktop ]] || fail "Desktop file was not removed"
  [[ ! -f /usr/share/icons/hicolor/scalable/apps/soundthis.svg ]] || fail "SVG icon was not removed"
  [[ ! -d /usr/share/omarchy/shell/plugins/soundthis ]] || fail "Plugin directory was not removed"
  success "All /usr files cleanly removed"

  rm -rf "$BUILD_DIR"
  success "Complete package lifecycle test PASSED in Arch Linux container!"
  exit 0
fi

# Case 3: Local sandbox test (rootless, non-destructive, works on host)
info "Running local sandbox package verification & lifecycle test..."

SANDBOX_DIR=$(mktemp -d "/tmp/soundthis_e2e_XXXXXX")
trap 'rm -rf "$SANDBOX_DIR"' EXIT

info "1. Validating package files and metadata..."
[[ -f "$REPO_ROOT/pkg/PKGBUILD" ]] || fail "pkg/PKGBUILD missing"
[[ -f "$REPO_ROOT/pkg/soundthis.install" ]] || fail "pkg/soundthis.install missing"
[[ -f "$REPO_ROOT/pkg/soundthis.desktop" ]] || fail "pkg/soundthis.desktop missing"
[[ -f "$REPO_ROOT/pkg/90-soundthis.preset" ]] || fail "pkg/90-soundthis.preset missing"
[[ -f "$REPO_ROOT/pkg/soundthis.svg" ]] || fail "pkg/soundthis.svg missing"
success "Package source files present"

info "2. Checking desktop entry and systemd preset syntax..."
grep -q "\[Desktop Entry\]" "$REPO_ROOT/pkg/soundthis.desktop" || fail "Invalid desktop file"
grep -q "enable soundthis.service" "$REPO_ROOT/pkg/90-soundthis.preset" || fail "Invalid preset file"
success "Desktop entry and systemd preset validated"

info "3. Testing scriptlet lifecycle against mock Omarchy environment..."
BIN_DIR="$SANDBOX_DIR/bin"
mkdir -p "$BIN_DIR"
SHELL_JSON="$SANDBOX_DIR/shell.json"
echo '{"bar":{"layout":{"right":[{"id":"omarchy.tray"}]}}}' > "$SHELL_JSON"

export MOCK_SHELL_JSON="$SHELL_JSON"

cat << 'EOF' > "$BIN_DIR/omarchy-shell"
#!/usr/bin/env bash
export PATH="/usr/local/bin:/usr/bin:/bin:$PATH"
case "$*" in
  *enablePlugin*soundthis*)
    jq 'if (.bar.layout.right | any(.id == "soundthis" or . == "soundthis")) then . else .bar.layout.right = [{"id": "soundthis"}] + .bar.layout.right end' "$MOCK_SHELL_JSON" > "$MOCK_SHELL_JSON.tmp" && mv "$MOCK_SHELL_JSON.tmp" "$MOCK_SHELL_JSON"
    echo "ok"
    ;;
  *setPluginEnabled*soundthis*false*)
    jq '.bar.layout.right = [.bar.layout.right[] | select(.id != "soundthis" and . != "soundthis")]' "$MOCK_SHELL_JSON" > "$MOCK_SHELL_JSON.tmp" && mv "$MOCK_SHELL_JSON.tmp" "$MOCK_SHELL_JSON"
    echo "ok"
    ;;
esac
EOF
chmod +x "$BIN_DIR/omarchy-shell"

cat << 'EOF' > "$BIN_DIR/sudo"
#!/usr/bin/env bash
while [[ "${1:-}" == -* ]]; do
  if [[ "$1" == "-u" ]]; then shift 2; else shift 1; fi
done
exec "$@"
EOF
chmod +x "$BIN_DIR/sudo"

bash -c "
  set -eu
  export PATH=\"$BIN_DIR:\$PATH\"
  source '$REPO_ROOT/pkg/soundthis.install'
  _get_target_users() { echo '1000'; }

  # Test post_install
  post_install
  jq -e 'any(.bar.layout.right[]; .id == \"soundthis\" or . == \"soundthis\")' '$SHELL_JSON' >/dev/null

  # Test post_install idempotency
  post_install
  count=\$(jq '[.bar.layout.right[] | select(.id == \"soundthis\" or . == \"soundthis\")] | length' '$SHELL_JSON')
  [[ \"\$count\" -eq 1 ]]

  # Test pre_remove
  pre_remove
  if jq -e 'any(.bar.layout.right[]; .id == \"soundthis\" or . == \"soundthis\")' '$SHELL_JSON' >/dev/null 2>&1; then
    exit 1
  fi

  # Test post_remove
  post_remove
"

success "Scriptlet post_install, idempotency, pre_remove, and post_remove passed"

if command -v makepkg >/dev/null 2>&1; then
  info "4. Testing package build with makepkg in sandbox..."
  cp -r "$REPO_ROOT/pkg"/* "$SANDBOX_DIR"/
  (cd "$SANDBOX_DIR" && makepkg -fd --nodeps >/dev/null 2>&1)
  PKG_ARCHIVE=$(find "$SANDBOX_DIR" -maxdepth 1 -name "soundthis-*.pkg.tar.zst" | head -n1)
  if [[ -f "$PKG_ARCHIVE" ]]; then
    success "Package built cleanly: $(basename "$PKG_ARCHIVE")"
    info "Inspecting archive contents..."
    ARCHIVE_LIST=$(tar -tf "$PKG_ARCHIVE")
    echo "$ARCHIVE_LIST" | grep -q "usr/bin/soundthis" || fail "Binary missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/lib/systemd/user/soundthis.service" || fail "Service missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/lib/systemd/user-preset/90-soundthis.preset" || fail "Preset missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/share/applications/soundthis.desktop" || fail "Desktop entry missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/share/icons/hicolor/scalable/apps/soundthis.svg" || fail "Icon missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/share/omarchy/shell/plugins/soundthis/manifest.json" || fail "Manifest missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/share/omarchy/shell/plugins/soundthis/Panel.qml" || fail "Panel.qml missing from built package"
    success "All package assets verified inside .pkg.tar.zst archive"
  fi
fi

success "Local package lifecycle sandbox test PASSED!"
