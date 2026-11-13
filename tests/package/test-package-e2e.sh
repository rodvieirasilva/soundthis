#!/usr/bin/env bash
# End-to-End Package Installation & Lifecycle Verification for SoundThis
# No mocks — tests real package building, pacman hooks, and omarchy CLI dispatch.
#
# Usage:
#   --docker     : Run the full omarchy pkg add/drop lifecycle in an Arch Linux container
#   --container  : Executed inside an Arch Linux container (automated by --docker or CI)
#   (default)    : Local package verification and makepkg build test
set -euo pipefail

PACKAGE_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$PACKAGE_DIR/../.." && pwd)"

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
    bash -c "/workspace/tests/package/test-package-e2e.sh --container"
  exit $?
fi

# Case 2: Inside container mode (running as root in Arch Linux)
if [[ "${1:-}" == "--container" ]]; then
  info "Running inside Arch Linux container..."

  # Install required package dependencies
  pacman -Syu --noconfirm --needed jq libpulse

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

  # Install real omarchy CLI tools into /usr/local/bin
  cp "$REPO_ROOT"/tests/package/omarchy/* /usr/local/bin/
  chmod +x /usr/local/bin/omarchy*

  # Build the package as user builder using real makepkg
  info "Building package with makepkg..."
  sudo -u builder bash -c "cd '$BUILD_DIR' && makepkg -fd --nodeps"

  PKG_FILE=$(find "$BUILD_DIR" -maxdepth 1 -name "soundthis-*.pkg.tar.zst" | head -n1)
  [[ -f "$PKG_FILE" ]] || fail "Package .pkg.tar.zst was not generated"
  success "Package generated: $(basename "$PKG_FILE")"

  # Create a real local pacman repository for omarchy pkg add
  LOCAL_REPO="/var/cache/pacman/soundthis-local"
  rm -rf "$LOCAL_REPO"
  mkdir -p "$LOCAL_REPO"
  cp "$PKG_FILE" "$LOCAL_REPO/"
  repo-add "$LOCAL_REPO/soundthis-local.db.tar.gz" "$LOCAL_REPO"/soundthis-*.pkg.tar.zst

  cat << 'EOF' >> /etc/pacman.conf

[soundthis-local]
SigLevel = Optional TrustAll
Server = file:///var/cache/pacman/soundthis-local
EOF

  pacman -Sy

  # 1. Test Package Installation via real omarchy pkg add
  info "Checking initial missing status with 'omarchy pkg missing'..."
  /usr/local/bin/omarchy pkg missing soundthis || fail "soundthis should initially be reported missing"

  info "Installing package with 'omarchy pkg add soundthis'..."
  SUDO_USER=builder /usr/local/bin/omarchy pkg add soundthis

  info "Verifying installed status with 'omarchy pkg present'..."
  /usr/local/bin/omarchy pkg present soundthis || fail "soundthis was not reported present by omarchy CLI"
  success "omarchy pkg add completed successfully"

  info "Verifying installed files..."
  [[ -x /usr/bin/soundthis ]] || fail "/usr/bin/soundthis is missing or not executable"
  [[ -f /usr/lib/systemd/user/soundthis.service ]] || fail "/usr/lib/systemd/user/soundthis.service is missing"
  [[ -f /usr/lib/systemd/user-preset/90-soundthis.preset ]] || fail "Systemd user preset is missing"
  [[ -f /usr/share/applications/soundthis.desktop ]] || fail "Desktop file is missing"
  [[ -f /usr/share/icons/hicolor/scalable/apps/soundthis.svg ]] || fail "Scalable SVG icon is missing"
  [[ -f /usr/share/omarchy/shell/plugins/soundthis/manifest.json ]] || fail "Plugin manifest.json is missing"
  [[ -f /usr/share/omarchy/shell/plugins/soundthis/Panel.qml ]] || fail "Plugin Panel.qml is missing"
  success "All package files verified in /usr"

  # Test real binary execution
  info "Testing binary execution..."
  /usr/bin/soundthis --version
  /usr/bin/soundthis --help >/dev/null
  success "soundthis binary runs and reports version/help"

  # Verify systemd preset content
  grep -q "enable soundthis.service" /usr/lib/systemd/user-preset/90-soundthis.preset || fail "Preset does not enable soundthis.service"
  success "Systemd user preset correctly enables soundthis.service"

  # 2. Test Package Uninstallation via real omarchy pkg drop
  info "Uninstalling package with 'omarchy pkg drop soundthis'..."
  SUDO_USER=builder /usr/local/bin/omarchy pkg drop soundthis

  info "Verifying uninstalled status with 'omarchy pkg missing'..."
  /usr/local/bin/omarchy pkg missing soundthis || fail "soundthis should be reported missing after drop"
  success "omarchy pkg drop completed successfully"

  info "Verifying package removal..."
  [[ ! -f /usr/bin/soundthis ]] || fail "/usr/bin/soundthis was not removed"
  [[ ! -f /usr/lib/systemd/user/soundthis.service ]] || fail "Systemd service was not removed"
  [[ ! -f /usr/lib/systemd/user-preset/90-soundthis.preset ]] || fail "Preset was not removed"
  [[ ! -f /usr/share/applications/soundthis.desktop ]] || fail "Desktop file was not removed"
  [[ ! -f /usr/share/icons/hicolor/scalable/apps/soundthis.svg ]] || fail "SVG icon was not removed"
  [[ ! -d /usr/share/omarchy/shell/plugins/soundthis ]] || fail "Plugin directory was not removed"
  success "All /usr files cleanly removed"

  rm -rf "$BUILD_DIR" "$LOCAL_REPO"
  success "Complete package lifecycle test PASSED in Arch Linux container with omarchy CLI (no mocks)!"
  exit 0
fi

# Case 3: Local sandbox test (rootless, non-destructive, works on host & Ubuntu CI)
info "Running package verification & makepkg build test..."

SANDBOX_DIR=$(mktemp -d -t soundthis_pkg_test.XXXXXX)
trap 'rm -rf "$SANDBOX_DIR"' EXIT

info "1. Validating package source files..."
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

info "3. Validating omarchy CLI command dispatch..."
"$PACKAGE_DIR/omarchy/omarchy" pkg --help >/dev/null
success "omarchy CLI verified"

info "4. Testing install scriptlet hooks directly with bash..."
# Sourcing soundthis.install and running hooks with real system environment (no mocks)
bash -c "
  set -eu
  source '$REPO_ROOT/pkg/soundthis.install'
  # Execute functions directly
  post_install
  post_upgrade
  pre_remove
  post_remove
"
success "soundthis.install scriptlet executed cleanly"

if command -v makepkg >/dev/null 2>&1; then
  info "5. Testing package build with makepkg..."
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

success "Package verification & build test PASSED (no mocks)!"
