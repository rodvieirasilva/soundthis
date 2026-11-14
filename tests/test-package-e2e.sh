#!/usr/bin/env bash
# End-to-End Package Installation & Lifecycle Verification for SoundThis
# No mocks — tests real package building, pacman hooks, and omarchy CLI if available.
#
# Usage:
#   --docker     : Run the full package lifecycle in an Arch Linux container
#   --container  : Executed inside an Arch Linux container (automated by --docker or CI)
#   (default)    : Local package verification and makepkg build test
set -euo pipefail

TESTS_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$TESTS_DIR/.." && pwd)"

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
    bash -c "/workspace/tests/test-package-e2e.sh --container"
  exit $?
fi

# Case 2: Inside container mode (running as root in Arch Linux)
if [[ "${1:-}" == "--container" ]]; then
  info "Running inside Arch Linux container..."

  # Install required package dependencies
  pacman -Syu --noconfirm --needed jq libpulse gtk4 libadwaita

  # Setup unprivileged build user for makepkg
  if ! id -u builder >/dev/null 2>&1; then
    useradd -m -s /bin/bash builder
    echo "builder ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers
  fi

  BUILD_DIR="/tmp/soundthis_pkg_build"
  rm -rf "$BUILD_DIR"
  mkdir -p "$BUILD_DIR"
  cp -r "$REPO_ROOT"/pkg/* "$BUILD_DIR"/

  # Stage local release archive if built, so makepkg builds real local artifacts
  ARCH=$(uname -m)
  PKGVER=$(grep -E '^pkgver=' "$BUILD_DIR/PKGBUILD" | cut -d'=' -f2)
  ARCHIVE="soundthis-${PKGVER}-${ARCH}-linux.tar.gz"
  if [[ -f "$REPO_ROOT/target/release/soundthis" ]]; then
    STAGE_DIR=$(mktemp -d)
    mkdir -p "$STAGE_DIR/package"
    cp "$REPO_ROOT/target/release/soundthis" "$STAGE_DIR/package/"
    cp "$REPO_ROOT/README.md" "$STAGE_DIR/package/" 2>/dev/null || true
    cp "$REPO_ROOT/LICENSE" "$STAGE_DIR/package/" 2>/dev/null || true
    cp -r "$REPO_ROOT/systemd" "$STAGE_DIR/package/" 2>/dev/null || true
    cp -r "$REPO_ROOT/plugin" "$STAGE_DIR/package/" 2>/dev/null || true
    cp -r "$REPO_ROOT/config" "$STAGE_DIR/package/" 2>/dev/null || true
    cp -r "$REPO_ROOT/assets" "$STAGE_DIR/package/" 2>/dev/null || true
    cp -r "$REPO_ROOT/docs" "$STAGE_DIR/package/" 2>/dev/null || true
    tar -czf "$BUILD_DIR/$ARCHIVE" -C "$STAGE_DIR/package" .
    rm -rf "$STAGE_DIR"
    NEW_SUM=$(sha256sum "$BUILD_DIR/$ARCHIVE" | awk '{print $1}')
    if [[ "$ARCH" == "x86_64" ]]; then
      sed -i "s|sha256sums_x86_64=('.*')|sha256sums_x86_64=('$NEW_SUM')|" "$BUILD_DIR/PKGBUILD"
    elif [[ "$ARCH" == "aarch64" ]]; then
      sed -i "s|sha256sums_aarch64=('.*')|sha256sums_aarch64=('$NEW_SUM')|" "$BUILD_DIR/PKGBUILD"
    fi
  fi
  chown -R builder:builder "$BUILD_DIR"

  # Build the package as user builder using real makepkg
  info "Building package with makepkg..."
  sudo -u builder bash -c "cd '$BUILD_DIR' && makepkg -fd --nodeps"

  PKG_FILE=$(find "$BUILD_DIR" -maxdepth 1 -name "soundthis-*.pkg.tar.zst" | head -n1)
  [[ -f "$PKG_FILE" ]] || fail "Package .pkg.tar.zst was not generated"
  success "Package generated: $(basename "$PKG_FILE")"

  # 1. Test Package Installation
  info "Installing package with pacman -U..."
  SUDO_USER=builder pacman -U --noconfirm --nodeps "$PKG_FILE"

  if command -v omarchy >/dev/null 2>&1; then
    info "Verifying package with 'omarchy pkg present'..."
    omarchy pkg present soundthis || fail "omarchy pkg present failed"
  fi
  success "Package installed successfully"

  info "Verifying installed files..."
  [[ -x /usr/bin/soundthis ]] || fail "/usr/bin/soundthis is missing or not executable"
  [[ -f /usr/lib/systemd/user/soundthis.service ]] || fail "/usr/lib/systemd/user/soundthis.service is missing"
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

  # 2. Test Package Uninstallation
  info "Uninstalling package with pacman -Rns..."
  if command -v omarchy >/dev/null 2>&1; then
    SUDO_USER=builder omarchy pkg drop soundthis
  else
    SUDO_USER=builder pacman -Rns --noconfirm --nodeps soundthis
  fi

  if command -v omarchy >/dev/null 2>&1; then
    omarchy pkg missing soundthis || fail "soundthis should be reported missing after drop"
  fi
  success "Package uninstalled successfully"

  info "Verifying package removal..."
  [[ ! -f /usr/bin/soundthis ]] || fail "/usr/bin/soundthis was not removed"
  [[ ! -f /usr/lib/systemd/user/soundthis.service ]] || fail "Systemd service was not removed"
  [[ ! -f /usr/share/applications/soundthis.desktop ]] || fail "Desktop file was not removed"
  [[ ! -f /usr/share/icons/hicolor/scalable/apps/soundthis.svg ]] || fail "SVG icon was not removed"
  [[ ! -d /usr/share/omarchy/shell/plugins/soundthis ]] || fail "Plugin directory was not removed"
  success "All /usr files cleanly removed"

  rm -rf "$BUILD_DIR"
  success "Complete package lifecycle test PASSED in Arch Linux container (zero mocks)!"
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
[[ -f "$REPO_ROOT/pkg/soundthis.svg" ]] || fail "pkg/soundthis.svg missing"
success "Package source files present"

info "2. Checking desktop entry syntax..."
grep -q "\[Desktop Entry\]" "$REPO_ROOT/pkg/soundthis.desktop" || fail "Invalid desktop file"
grep -q "Exec=/usr/bin/soundthis" "$REPO_ROOT/pkg/soundthis.desktop" || fail "Invalid Exec line in desktop file"
success "Desktop entry validated"

if command -v omarchy >/dev/null 2>&1; then
  info "3. Validating omarchy CLI integration on host..."
  omarchy pkg --help >/dev/null
  success "omarchy CLI verified"
fi

info "4. Testing install scriptlet hooks directly with bash..."
bash -c "
  set -eu
  source '$REPO_ROOT/pkg/soundthis.install'
  post_install
  post_upgrade
  pre_remove
  post_remove
"
success "soundthis.install scriptlet executed cleanly"

if command -v makepkg >/dev/null 2>&1; then
  info "5. Testing package build with makepkg..."
  cp -r "$REPO_ROOT/pkg"/* "$SANDBOX_DIR"/
  ARCH=$(uname -m)
  PKGVER=$(grep -E '^pkgver=' "$SANDBOX_DIR/PKGBUILD" | cut -d'=' -f2)
  ARCHIVE="soundthis-${PKGVER}-${ARCH}-linux.tar.gz"
  if [[ -f "$REPO_ROOT/target/release/soundthis" ]]; then
    STAGE_DIR=$(mktemp -d)
    mkdir -p "$STAGE_DIR/package"
    cp "$REPO_ROOT/target/release/soundthis" "$STAGE_DIR/package/"
    cp "$REPO_ROOT/README.md" "$STAGE_DIR/package/" 2>/dev/null || true
    cp "$REPO_ROOT/LICENSE" "$STAGE_DIR/package/" 2>/dev/null || true
    cp -r "$REPO_ROOT/systemd" "$STAGE_DIR/package/" 2>/dev/null || true
    cp -r "$REPO_ROOT/plugin" "$STAGE_DIR/package/" 2>/dev/null || true
    cp -r "$REPO_ROOT/config" "$STAGE_DIR/package/" 2>/dev/null || true
    cp -r "$REPO_ROOT/assets" "$STAGE_DIR/package/" 2>/dev/null || true
    cp -r "$REPO_ROOT/docs" "$STAGE_DIR/package/" 2>/dev/null || true
    tar -czf "$SANDBOX_DIR/$ARCHIVE" -C "$STAGE_DIR/package" .
    rm -rf "$STAGE_DIR"
    NEW_SUM=$(sha256sum "$SANDBOX_DIR/$ARCHIVE" | awk '{print $1}')
    if [[ "$ARCH" == "x86_64" ]]; then
      sed -i "s|sha256sums_x86_64=('.*')|sha256sums_x86_64=('$NEW_SUM')|" "$SANDBOX_DIR/PKGBUILD"
    elif [[ "$ARCH" == "aarch64" ]]; then
      sed -i "s|sha256sums_aarch64=('.*')|sha256sums_aarch64=('$NEW_SUM')|" "$SANDBOX_DIR/PKGBUILD"
    fi
  fi
  (cd "$SANDBOX_DIR" && makepkg -fd --nodeps >/dev/null 2>&1)
  PKG_ARCHIVE=$(find "$SANDBOX_DIR" -maxdepth 1 -name "soundthis-*.pkg.tar.zst" | head -n1)
  if [[ -f "$PKG_ARCHIVE" ]]; then
    success "Package built cleanly: $(basename "$PKG_ARCHIVE")"
    info "Inspecting archive contents..."
    ARCHIVE_LIST=$(tar -tf "$PKG_ARCHIVE")
    echo "$ARCHIVE_LIST" | grep -q "usr/bin/soundthis" || fail "Binary missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/lib/systemd/user/soundthis.service" || fail "Service missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/share/applications/soundthis.desktop" || fail "Desktop entry missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/share/icons/hicolor/scalable/apps/soundthis.svg" || fail "Icon missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/share/omarchy/shell/plugins/soundthis/manifest.json" || fail "Manifest missing from built package"
    echo "$ARCHIVE_LIST" | grep -q "usr/share/omarchy/shell/plugins/soundthis/Panel.qml" || fail "Panel.qml missing from built package"
    success "All package assets verified inside .pkg.tar.zst archive"
  fi
fi

success "Package verification & build test PASSED (no mocks)!"
