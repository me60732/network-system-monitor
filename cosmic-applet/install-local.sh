#!/bin/bash
# Install cosmic-applet as a COSMIC panel applet (system-wide, requires sudo).
# Run from the project root: ./cosmic-applet/install-local.sh

set -euo pipefail

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

echo "Installing icon to $INSTALL_ICONS/ ..."
sudo mkdir -p "$INSTALL_ICONS"
sudo install -m 644 \
    "$PROJECT_ROOT/cosmic-applet/res/icons/$APP_ID.svg" \
    "$INSTALL_ICONS/$APP_ID.svg"

echo "Updating icon cache and desktop database..."
sudo gtk-update-icon-cache /usr/share/icons/hicolor/ 2>/dev/null || true
sudo update-desktop-database "$INSTALL_DESKTOP" 2>/dev/null || true

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
echo "  sudo rm $INSTALL_ICONS/$APP_ID.svg"
