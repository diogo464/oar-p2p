#!/bin/bash
set -euo pipefail

# Script to build static binaries for Linux and macOS
# Can be run locally or in CI

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY_NAME="oar-p2p"

# Parse command line arguments
TARGET_OS="${1:-all}"  # linux, macos, or all
OUTPUT_DIR="${2:-$PROJECT_ROOT/target/release-static}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
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

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Function to build for Linux with musl
build_linux() {
    local arch="${1:-x86_64}"
    local target="${arch}-unknown-linux-musl"
    
    log_info "Building static binary for Linux ($arch)..."
    
    # Check if target is installed
    if ! rustup target list --installed | grep -q "$target"; then
        log_info "Installing Rust target: $target"
        rustup target add "$target"
    fi
    
    # Set environment variables for static linking
    export RUSTFLAGS="-C target-feature=+crt-static"
    
    # Build the binary
    log_info "Running cargo build for $target..."
    cargo build --release --target "$target" --target-dir "$PROJECT_ROOT/target"
    
    # Copy binary to output directory with platform suffix
    local output_name="${BINARY_NAME}-linux-${arch}"
    cp "target/$target/release/$BINARY_NAME" "$OUTPUT_DIR/$output_name"
    
    # Strip the binary to reduce size
    if command -v strip >/dev/null 2>&1; then
        log_info "Stripping binary to reduce size..."
        strip "$OUTPUT_DIR/$output_name"
    fi
    
    # Make it executable
    chmod +x "$OUTPUT_DIR/$output_name"
    
    log_info "Linux binary built: $OUTPUT_DIR/$output_name"
    
    # Print binary info
    if command -v file >/dev/null 2>&1; then
        file "$OUTPUT_DIR/$output_name"
    fi
    
    # Check if it's actually statically linked
    if command -v ldd >/dev/null 2>&1; then
        if ldd "$OUTPUT_DIR/$output_name" 2>&1 | grep -q "not a dynamic executable"; then
            log_info "✓ Binary is statically linked"
        else
            log_warn "Binary appears to be dynamically linked"
        fi
    fi
}

# Function to build for macOS
build_macos() {
    local arch="${1:-x86_64}"
    local target
    
    case "$arch" in
        x86_64)
            target="x86_64-apple-darwin"
            ;;
        aarch64|arm64)
            target="aarch64-apple-darwin"
            ;;
        *)
            log_error "Unknown macOS architecture: $arch"
            return 1
            ;;
    esac
    
    # Check if we're on macOS
    if [[ "$OSTYPE" != "darwin"* ]]; then
        log_warn "Cannot build macOS binaries on non-macOS systems"
        log_info "macOS binaries must be built on macOS or in CI"
        return 0
    fi
    
    log_info "Building binary for macOS ($arch)..."
    
    # Check if target is installed
    if ! rustup target list --installed | grep -q "$target"; then
        log_info "Installing Rust target: $target"
        rustup target add "$target"
    fi
    
    # macOS doesn't support fully static binaries, but we can minimize dependencies
    export RUSTFLAGS="-C target-feature=+crt-static"
    
    # Build the binary
    log_info "Running cargo build for $target..."
    cargo build --release --target "$target" --target-dir "$PROJECT_ROOT/target"
    
    # Copy binary to output directory with platform suffix
    local output_name="${BINARY_NAME}-macos-${arch}"
    cp "target/$target/release/$BINARY_NAME" "$OUTPUT_DIR/$output_name"
    
    # Strip the binary to reduce size (macOS strip is different)
    if [[ "$OSTYPE" == "darwin"* ]] && command -v strip >/dev/null 2>&1; then
        log_info "Stripping binary to reduce size..."
        strip "$OUTPUT_DIR/$output_name"
    fi
    
    # Make it executable
    chmod +x "$OUTPUT_DIR/$output_name"
    
    log_info "macOS binary built: $OUTPUT_DIR/$output_name"
    
    # Print binary info
    if command -v file >/dev/null 2>&1; then
        file "$OUTPUT_DIR/$output_name"
    fi
}

# Function to check dependencies
check_dependencies() {
    local missing_deps=()
    
    # Check for Rust
    if ! command -v cargo >/dev/null 2>&1; then
        missing_deps+=("cargo (Rust)")
    fi
    
    # Check for rustup
    if ! command -v rustup >/dev/null 2>&1; then
        missing_deps+=("rustup")
    fi
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        log_error "Missing required dependencies:"
        for dep in "${missing_deps[@]}"; do
            echo "  - $dep"
        done
        exit 1
    fi
    
    # Check Rust version (nightly required based on Cargo.toml)
    if ! rustc --version | grep -q "nightly"; then
        log_warn "This project requires Rust nightly. Current version:"
        rustc --version
        log_info "Attempting to use nightly for this build..."
        export RUSTUP_TOOLCHAIN=nightly
    fi
}

# Main execution
main() {
    log_info "Starting static build process..."
    log_info "Output directory: $OUTPUT_DIR"
    
    # Check dependencies
    check_dependencies
    
    case "$TARGET_OS" in
        linux)
            build_linux "x86_64"
            # Optionally build for ARM64
            # build_linux "aarch64"
            ;;
        macos|darwin)
            # Build for both Intel and Apple Silicon
            build_macos "x86_64"
            build_macos "aarch64"
            ;;
        all)
            # Build for all platforms
            build_linux "x86_64"
            build_macos "x86_64"
            build_macos "aarch64"
            ;;
        *)
            log_error "Unknown target OS: $TARGET_OS"
            echo "Usage: $0 [linux|macos|all] [output_dir]"
            exit 1
            ;;
    esac
    
    log_info "Build complete! Binaries available in: $OUTPUT_DIR"
    ls -lh "$OUTPUT_DIR"
}

# Run main function
main