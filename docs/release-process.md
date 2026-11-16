# SoundThis Release & Versioning Guide

This document outlines the complete step-by-step process for cutting a new version release of **SoundThis**, publishing GitHub Release assets, and updating the package recipe in **Omarchy Linux** (`omacom/omarchy-pkgs`).

---

## Overview & Architecture

SoundThis distributes releases across two channels:
1. **GitHub Releases** (`rodvieirasilva/soundthis`): Pre-compiled standalone Linux tarballs with self-contained single binaries (`x86_64` and `aarch64`).
2. **Omarchy Official Package Repository** (`omacom/omarchy-pkgs`): Arch Linux package recipe (`PKGBUILD`) that downloads the GitHub release tarballs, verifies SHA256 checksums, and installs the binary, desktop launcher, and Omarchy shell top bar widget.

---

## Release Workflow Diagram

```
1. Local Quality Gates (lint, test, test:e2e, coverage)
                     │
2. Version Bumping across 8 files (Cargo.toml, lock, web, plugin, PKGBUILD, README)
                     │
3. Release PR (branch -> PR -> GitHub CI -> Merge to master)
                     │
4. Git Tag Push (vX.Y.Z -> triggers GitHub Actions release.yml)
                     │
5. GitHub Release Build (compiles x86_64 & aarch64, uploads tarballs & sha256)
                     │
6. Checksum Sync (record release sha256sums into pkg/PKGBUILD on master)
                     │
7. Omarchy PKG PR (update omacom/omarchy-pkgs with new version & checksums)
```

---

## Step 1: Pre-Release Quality Gates

All checks must pass with zero errors and zero warnings before cutting a release:

```bash
# 1. Check all linters (Rust Clippy with -D warnings, Rustfmt, ESLint)
mise run lint

# 2. Run all unit and integration test suites (Rust + Vitest)
mise run test

# 3. Run browser E2E test suite (Playwright: zero console errors, zero 404s)
mise run test:e2e

# 4. Verify test coverage remains >= 80% on both suites
mise run coverage
```

> [!NOTE]
> If UI or bar widget styling changed in this release, re-capture the documentation screenshots and live theme animation before releasing:
> ```bash
> mise run screenshots
> mise run gif
> ```

---

## Step 2: Version Bumping Checklist

A new version requires updating the version string across **8 coordinated files**:

| File | Location | Change |
|---|---|---|
| `Cargo.toml` | `[package] version` | Update to `"X.Y.Z"` |
| `Cargo.lock` | Root package entry | Run `cargo check` to update |
| `web/package.json` | `"version"` | Update to `"X.Y.Z"` |
| `web/index.html` | `<meta name="version">` | Update `content="vX.Y.Z"` |
| `plugin/manifest.json` | `"version"` | Update to `"X.Y.Z"` |
| `plugin/Panel.qml` | Hero subtitle `text` | Update to `"SoundThis vX.Y.Z"` |
| `pkg/PKGBUILD` | `pkgver` & `pkgrel` | Update `pkgver=X.Y.Z` and reset `pkgrel=1` |
| `README.md` | Download links / snippets | Update references from `vA.B.C` to `vX.Y.Z` |

Verify all occurrences are updated:
```bash
git grep "v0.5.0"   # Replace with old version to confirm none were missed
git diff
```

---

## Step 3: Release PR & Merging to Master

Create a dedicated release branch and submit a pull request:

```bash
# Create release branch
git checkout -b feat/vX.Y.Z-release

# Commit and push
git commit -am "chore(release): bump version to vX.Y.Z"
git push origin feat/vX.Y.Z-release

# Open PR using GitHub CLI
gh pr create \
  --title "feat: release vX.Y.Z" \
  --body "Release vX.Y.Z with latest features and dependency updates."
```

Wait for GitHub Actions CI (`ci.yml`) to pass on the PR. Once green, merge the PR into `master`:

```bash
gh pr merge --squash --delete-branch
```

---

## Step 4: Tagging & Publishing GitHub Release

Pull the merged `master` branch locally, create an annotated git tag, and push:

```bash
git checkout master
git pull origin master

# Create annotated tag matching semver with 'v' prefix
git tag -a vX.Y.Z -m "Release vX.Y.Z"

# Push tag to trigger release workflow
git push origin vX.Y.Z
```

Pushing `v*` tags automatically triggers `.github/workflows/release.yml`:
1. **`build-packages` (matrix)**:
   - Compiles headless and GUI-ready binary on `x86_64` (Ubuntu)
   - Compiles release binary on `aarch64` (Ubuntu ARM runner)
   - Packages `soundthis-vX.Y.Z-<arch>-linux.tar.gz` and creates `.sha256` files
2. **`publish-release`**:
   - Collects artifacts from both build jobs
   - Publishes the official GitHub Release with tarballs and checksums attached

Monitor the workflow:
```bash
gh run list --workflow=release.yml --limit 1
# Or watch the latest run:
gh run watch $(gh run list --workflow=release.yml --limit 1 --json databaseId -q '.[0].databaseId')
```

---

## Step 5: Checksum Sync in Local PKGBUILD

Once the release workflow finishes, download the computed `.sha256` checksum files:

```bash
# Download checksum assets
gh release download vX.Y.Z -p "*.sha256" --dir /tmp/soundthis-release
cat /tmp/soundthis-release/*.sha256
```

Update the checksums in `pkg/PKGBUILD`:
```bash
# In pkg/PKGBUILD:
sha256sums_x86_64=('<HASH_FROM_x86_64.tar.gz.sha256>')
sha256sums_aarch64=('<HASH_FROM_aarch64.tar.gz.sha256>')
```

Commit and push directly to `master`:
```bash
git commit -am "chore(pkg): update sha256sums for vX.Y.Z release tarballs"
git push origin master
```

---

## Step 6: Upstream Omarchy Packaging PR (`omacom/omarchy-pkgs`)

Omarchy Linux maintains the official recipe in the `omacom/omarchy-pkgs` repository.

### 1. Synchronize the Packaging Workspace
Navigate to your clone of `omarchy-pkgs` (e.g., `~/dev/omarchy-pkgs`):
```bash
cd ~/dev/omarchy-pkgs
git checkout master
git pull upstream master
```

Switch to your feature branch (e.g., `add-soundthis` or `update-soundthis-vX.Y.Z`):
```bash
git checkout -b update-soundthis-vX.Y.Z
```

### 2. Update `pkgbuilds/soundthis/PKGBUILD`
Edit `pkgbuilds/soundthis/PKGBUILD`:
- Set `pkgver=X.Y.Z`
- Reset `pkgrel=1`
- Update `sha256sums_x86_64` with the new x86_64 checksum
- Update `sha256sums_aarch64` with the new aarch64 checksum

Verify non-binary static source checksums (`soundthis.desktop`, `soundthis.svg`):
```bash
sha256sum pkgbuilds/soundthis/soundthis.desktop pkgbuilds/soundthis/soundthis.svg
```

### 3. Verify Package Metadata
```bash
bash -c 'source helpers/package-metadata.sh && validate_package_metadata pkgbuilds/soundthis'
```

### 4. Test Clean Package Build
Execute a real packaging build with `makepkg` to ensure archive integrity:
```bash
cd pkgbuilds/soundthis
makepkg -C -c -f --nodeps

# Inspect package file structure:
tar -tvf soundthis-*.pkg.tar.zst

# Clean up build artifacts before committing:
rm -rf src pkg soundthis-*.pkg.tar.zst soundthis-*.tar.gz
cd ../..
```

### 5. Commit, Push & Submit/Update PR
```bash
git commit -am "feat(soundthis): update package to vX.Y.Z"
git push origin <branch>
```

#### If opening a new PR:
```bash
gh pr create \
  --repo omacom/omarchy-pkgs \
  --base master \
  --title "Add soundthis, real-time desktop audio streaming server" \
  --body "### Summary

This PR adds the package recipe for SoundThis (\`soundthis\`), an ultra-low latency real-time desktop audio streaming server built natively for Omarchy Linux in Rust and TypeScript/Vite.

**Upstream Repository**: https://github.com/rodvieirasilva/soundthis

### Architecture & Features (vX.Y.Z)
• Single unified executable containing server, web client, and native GTK4/libadwaita window
• Event-driven 0ms theme syncing with Omarchy shell palettes
• Multi-architecture release tarballs for x86_64 and aarch64
• Integrated Omarchy shell top bar widget and popout panel

Validated against makepkg lifecycle, package tests, and omarchy CLI."
```

#### If updating an existing open PR:
```bash
gh pr comment <PR_NUMBER> --repo omacom/omarchy-pkgs --body "Updated to vX.Y.Z:
• Pinned release checksums for upstream vX.Y.Z tarballs
• Updated changelog and verified clean makepkg packaging build."
```

---

## Release Checklist Summary

- [ ] All pre-release tests pass (`mise run lint`, `mise run test`, `mise run test:e2e`, `mise run coverage`).
- [ ] Version bumped across 8 coordinated files.
- [ ] Release PR merged into `master`.
- [ ] Git tag `vX.Y.Z` created and pushed.
- [ ] GitHub Actions release workflow finished with published assets.
- [ ] Checksums updated in `soundthis/pkg/PKGBUILD`.
- [ ] `omacom/omarchy-pkgs` PR updated and verified with `makepkg`.
