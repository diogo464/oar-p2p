#!/bin/bash
set -euo pipefail

# Script to prepare release artifacts
# Builds binaries, creates checksums, and prepares release notes

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY_NAME="oar-p2p"

# Parse command line arguments
VERSION="${1:-}"
OUTPUT_DIR="${2:-$PROJECT_ROOT/dist}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# Ensure we're in the project root
cd "$PROJECT_ROOT"

# Check if version is provided
if [ -z "$VERSION" ]; then
    log_error "Version not provided"
    echo "Usage: $0 <version> [output_dir]"
    echo "Example: $0 v1.0.0"
    exit 1
fi

# Remove 'v' prefix if present for consistency
VERSION_NUMBER="${VERSION#v}"

log_info "Preparing release for version: $VERSION_NUMBER"

# Create output directories
BUILD_DIR="$PROJECT_ROOT/target/release-static"
mkdir -p "$OUTPUT_DIR"
mkdir -p "$BUILD_DIR"

# Build all binaries
log_info "Building static binaries for all platforms..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    # On macOS, build all platforms
    "$SCRIPT_DIR/build-static.sh" all "$BUILD_DIR"
else
    # On Linux, only build Linux binaries
    log_warn "Running on Linux - only Linux binaries will be built"
    log_info "macOS binaries must be built on macOS or in CI"
    "$SCRIPT_DIR/build-static.sh" linux "$BUILD_DIR"
fi

# Function to create tarball for a binary
create_tarball() {
    local binary_path="$1"
    local binary_name="$(basename "$binary_path")"
    local platform_name="${binary_name#$BINARY_NAME-}"
    local tarball_name="${BINARY_NAME}-${VERSION_NUMBER}-${platform_name}.tar.gz"
    
    log_info "Creating tarball: $tarball_name"
    
    # Create temporary directory for tarball contents
    local temp_dir="$(mktemp -d)"
    
    # Copy binary to temp directory
    cp "$binary_path" "$temp_dir/$BINARY_NAME"
    chmod +x "$temp_dir/$BINARY_NAME"
    
    # Create README for the tarball
    cat > "$temp_dir/README.md" << EOF
# $BINARY_NAME v$VERSION_NUMBER

Platform: ${platform_name}

## Installation

1. Extract the binary:
   \`\`\`bash
   tar -xzf $tarball_name
   \`\`\`

2. Move to a directory in your PATH:
   \`\`\`bash
   sudo mv $BINARY_NAME /usr/local/bin/
   \`\`\`

3. Verify installation:
   \`\`\`bash
   $BINARY_NAME --version
   \`\`\`

## Usage

Run \`$BINARY_NAME --help\` for usage information.

EOF
    
    # Create tarball
    tar -czf "$OUTPUT_DIR/$tarball_name" -C "$temp_dir" .
    
    # Cleanup
    rm -rf "$temp_dir"
    
    return 0
}

# Create tarballs for all binaries
log_info "Creating release tarballs..."
for binary in "$BUILD_DIR"/${BINARY_NAME}-*; do
    if [ -f "$binary" ]; then
        create_tarball "$binary"
    fi
done

# Generate checksums
log_info "Generating checksums..."
cd "$OUTPUT_DIR"

# Create SHA256 checksums
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum *.tar.gz > "checksums-sha256.txt"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 *.tar.gz > "checksums-sha256.txt"
else
    log_warn "SHA256 checksum tool not found, skipping checksums"
fi

# Create SHA512 checksums
if command -v sha512sum >/dev/null 2>&1; then
    sha512sum *.tar.gz > "checksums-sha512.txt"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 512 *.tar.gz > "checksums-sha512.txt"
fi

# Generate release notes template
log_info "Generating release notes template..."
cat > "$OUTPUT_DIR/RELEASE_NOTES.md" << EOF
# Release Notes - $BINARY_NAME v$VERSION_NUMBER

## What's Changed

<!-- Add your changes here -->

## Installation

### Using curl (Linux/macOS)

\`\`\`bash
# Linux x86_64
curl -L https://github.com/diogo464/oar-p2p/releases/download/v$VERSION_NUMBER/${BINARY_NAME}-${VERSION_NUMBER}-linux-x86_64.tar.gz | tar -xz
sudo mv $BINARY_NAME /usr/local/bin/

# macOS Intel
curl -L https://github.com/diogo464/oar-p2p/releases/download/v$VERSION_NUMBER/${BINARY_NAME}-${VERSION_NUMBER}-macos-x86_64.tar.gz | tar -xz
sudo mv $BINARY_NAME /usr/local/bin/

# macOS Apple Silicon
curl -L https://github.com/diogo464/oar-p2p/releases/download/v$VERSION_NUMBER/${BINARY_NAME}-${VERSION_NUMBER}-macos-aarch64.tar.gz | tar -xz
sudo mv $BINARY_NAME /usr/local/bin/
\`\`\`

## Checksums

### SHA256
\`\`\`
$(cat checksums-sha256.txt 2>/dev/null || echo "Checksums will be generated during release")
\`\`\`

### SHA512
\`\`\`
$(cat checksums-sha512.txt 2>/dev/null || echo "Checksums will be generated during release")
\`\`\`

## Full Changelog

See the full changelog at: https://github.com/diogo464/oar-p2p/compare/v<PREVIOUS_VERSION>...v$VERSION_NUMBER

EOF

# List all artifacts
log_info "Release artifacts created in: $OUTPUT_DIR"
echo -e "${BLUE}Contents:${NC}"
ls -lh "$OUTPUT_DIR"

# Print summary
echo
log_info "Release preparation complete!"
echo -e "${BLUE}Next steps:${NC}"
echo "1. Review and edit the release notes: $OUTPUT_DIR/RELEASE_NOTES.md"
echo "2. Create a git tag: git tag -a v$VERSION_NUMBER -m \"Release v$VERSION_NUMBER\""
echo "3. Push the tag: git push origin v$VERSION_NUMBER"
echo "4. The GitHub Action will automatically create the release and upload artifacts"