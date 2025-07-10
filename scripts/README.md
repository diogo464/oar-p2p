# Build Scripts

This directory contains scripts for building and releasing the oar-p2p binary.

## Scripts

### build-static.sh

Builds statically linked binaries for Linux and macOS.

```bash
# Build for all platforms (macOS only)
./scripts/build-static.sh all

# Build for Linux only
./scripts/build-static.sh linux

# Build for macOS only (macOS only)
./scripts/build-static.sh macos

# Specify custom output directory
./scripts/build-static.sh linux /path/to/output
```

**Note**: Cross-compilation is not supported. Linux binaries must be built on Linux, and macOS binaries must be built on macOS. The GitHub Actions workflow handles this by using different runners.

### prepare-release.sh

Prepares a release by building binaries, creating tarballs, and generating checksums.

```bash
# Prepare release for version v1.0.0
./scripts/prepare-release.sh v1.0.0

# Specify custom output directory
./scripts/prepare-release.sh v1.0.0 /path/to/dist
```

The script will:
1. Build static binaries for the current platform
2. Create tarballs with the binary and README
3. Generate SHA256 and SHA512 checksums
4. Create a release notes template

## GitHub Actions

The `.github/workflows/release.yml` workflow automatically:
1. Triggers on version tags (e.g., `v1.0.0`)
2. Builds binaries on Linux and macOS runners
3. Creates a GitHub release with all artifacts

To create a release:
```bash
git tag -a v1.0.0 -m "Release v1.0.0"
git push origin v1.0.0
```