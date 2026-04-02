#!/usr/bin/env bash
set -eo pipefail

BOLD=$(printf '\033[1m')
GREEN=$(printf '\033[38;2;166;227;161m')
PEACH=$(printf '\033[38;2;250;179;135m')
RESET=$(printf '\033[0m')

echo ""
echo "  ${BOLD}notez${RESET} installer"
echo ""

# Check for Rust toolchain
if ! command -v cargo &>/dev/null; then
    echo "  ${PEACH}✗${RESET} cargo not found. Install Rust: https://rustup.rs"
    exit 1
fi
echo "  ${GREEN}✓${RESET} cargo found"

# Determine install directory
INSTALL_DIR="${1:-$HOME/.local/bin}"
echo "  ${GREEN}✓${RESET} install directory: $INSTALL_DIR"

# Build release binary
echo ""
echo "  Building notez..."
cargo build --release --quiet

# Install
mkdir -p "$INSTALL_DIR"
cp target/release/notez "$INSTALL_DIR/notez"
chmod +x "$INSTALL_DIR/notez"

echo ""
echo "  ${GREEN}✓${RESET} installed to $INSTALL_DIR/notez"

# Check PATH
if ! echo "$PATH" | tr ':' '\n' | grep -q "^$INSTALL_DIR$"; then
    echo ""
    echo "  ${PEACH}!${RESET} $INSTALL_DIR is not in your PATH"
    echo "    Add this to your shell config:"
    echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo ""
echo "  Run ${BOLD}notez setup${RESET} to get started."
echo ""
