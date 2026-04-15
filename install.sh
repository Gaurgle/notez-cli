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

# Re-sign on macOS — `cp` over an existing Mach-O invalidates the ad-hoc
# linker signature, which causes the kernel to SIGKILL the new binary on
# launch. Re-applying an ad-hoc signature is harmless on Linux (no-op).
if command -v codesign &>/dev/null; then
    codesign --force --sign - "$INSTALL_DIR/notez" 2>/dev/null || true
fi

# Create symlinks for standalone commands.
# Naming convention:
#   z<verb>  for write/append commands  (zlog, znote, editz)
#   <noun>z  for view/manage TUIs       (todoz, logz, treez, findz)
for cmd in todoz zlog logz znote treez editz findz; do
    ln -sf "$INSTALL_DIR/notez" "$INSTALL_DIR/$cmd"
done

echo ""
echo "  ${GREEN}✓${RESET} installed to $INSTALL_DIR/notez"
echo "  ${GREEN}✓${RESET} standalone commands: todoz, zlog, logz, znote, treez, editz, findz"

# Check PATH
if ! echo "$PATH" | tr ':' '\n' | grep -q "^$INSTALL_DIR$"; then
    echo ""
    echo "  ${PEACH}!${RESET} $INSTALL_DIR is not in your PATH"
    echo "    Add this to your shell config:"
    echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo ""
echo "  To enable tab completions (zsh):"
echo "    mkdir -p ~/.zfunc"
echo "    notez completions zsh > ~/.zfunc/_notez"
echo "    # Add to .zshrc: fpath=(~/.zfunc \$fpath)"
echo ""
echo "  Run ${BOLD}notez setup${RESET} to get started."
echo ""
