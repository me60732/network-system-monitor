#!/bin/bash
# Install cosmic-applet as a COSMIC panel applet system-wide.
# Run from the project root AS YOURSELF (not via sudo) — it elevates per step:
#   ./cosmic-applet/install-local.sh

set -euo pipefail

# Run this as your own user — it elevates per command with `sudo` below. Prefixing the
# whole script with sudo puts the build on root's PATH, which has no cargo (the Rust
# toolchain lives in ~/.cargo/bin), so it dies before installing anything.
if [ "$(id -u)" -eq 0 ]; then
    echo "Do not run this script with sudo; it self-elevates. Re-run as your own user:"
    echo "    ./cosmic-applet/install-local.sh"
    exit 1
fi

# Non-login shells may not have ~/.cargo/bin on PATH yet.
if ! command -v cargo >/dev/null 2>&1 && [ -x "${HOME}/.cargo/bin/cargo" ]; then
    PATH="${HOME}/.cargo/bin:${PATH}"
fi
command -v cargo >/dev/null 2>&1 || {
    echo "cargo not found — install Rust (https://rustup.rs), then re-run without sudo."
    exit 1
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_ID="com.cosmic.network_system_monitor"
BINARY_NAME="cosmic-applet"
INSTALL_BIN="/usr/local/bin"
INSTALL_DESKTOP="/usr/share/applications"
INSTALL_ICONS="/usr/share/icons/hicolor/scalable/apps"

echo "Building release binary..."
cd "$PROJECT_ROOT"
cargo build --release -p cosmic-applet

echo "Installing binary to $INSTALL_BIN/ (requires sudo)..."
sudo install -m 755 target/release/cosmic-applet "$INSTALL_BIN/$BINARY_NAME"

echo "Installing desktop file to $INSTALL_DESKTOP/ ..."
# Use full binary path in Exec= so the panel can find it regardless of PATH
sudo bash -c "sed 's|Exec=com.cosmic.network_system_monitor|Exec=$INSTALL_BIN/$BINARY_NAME|' \
    '$PROJECT_ROOT/cosmic-applet/res/$APP_ID.desktop' \
    > '$INSTALL_DESKTOP/$APP_ID.desktop'"

echo "Installing icons to $INSTALL_ICONS/ ..."
sudo mkdir -p "$INSTALL_ICONS"
# The panel resolves sensor icons by freedesktop icon name, so every SVG in res/icons/
# must be installed — the app icon plus the per-sensor icons used by panel_widget.rs.
for svg in "$PROJECT_ROOT"/cosmic-applet/res/icons/*.svg; do
    sudo install -m 644 "$svg" "$INSTALL_ICONS/$(basename "$svg")"
done

echo "Updating icon cache and desktop database..."
sudo gtk-update-icon-cache /usr/share/icons/hicolor/ 2>/dev/null || true
sudo update-desktop-database "$INSTALL_DESKTOP" 2>/dev/null || true

# GPLv3 s4/s6: conveying the binary requires conveying a copy of the license with it.
sudo install -Dm644 "$PROJECT_ROOT/LICENSE" "/usr/share/doc/$APP_ID/LICENSE"

echo ""
echo "✓ Installed $APP_ID"
echo ""
echo "Next steps:"
echo "  1. Make sure ~/.config/cosmic-applet/config.toml exists"
echo "  2. Restart COSMIC panel:  kill \$(pgrep -o cosmic-panel)"
echo "  3. Go to COSMIC Settings → Desktop → Panel → Add Applet"
echo "  4. Find 'Network Monitor' in the list"
echo ""
echo "To uninstall:"
echo "  sudo rm $INSTALL_BIN/$BINARY_NAME"
echo "  sudo rm $INSTALL_DESKTOP/$APP_ID.desktop"
echo "  sudo rm $INSTALL_ICONS/$APP_ID*.svg"
