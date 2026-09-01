#!/usr/bin/env bash
# Network Monitor Daemon (nmd-service) Uninstallation Script

set -euo pipefail

SERVICE_NAME="nmd-service"
BINARY_PATH="/usr/local/bin/nmd-service"
SYSTEMD_UNIT="/etc/systemd/system/nmd.service"
CONFIG_DIR="/etc/nmd"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[✓]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
error() { echo -e "${RED}[✗]${NC} $*" >&2; }
die() { error "$*"; exit 1; }

[[ $EUID -eq 0 ]] || die "This script must be run as root (use sudo)"

warn "This will remove nmd from this machine."
read -p "Continue? [y/N] " -n 1 -r
echo
[[ $REPLY =~ ^[Yy]$ ]] || { log "Cancelled."; exit 0; }

# Stop and disable service
if systemctl is-active --quiet nmd; then
    log "Stopping nmd"
    systemctl stop nmd
fi

if systemctl is-enabled --quiet nmd 2>/dev/null; then
    log "Disabling nmd"
    systemctl disable nmd
fi

# Remove systemd unit
SYSTEMD_UNIT="/etc/systemd/system/nmd.service"
if [[ -f "$SYSTEMD_UNIT" ]]; then
    log "Removing systemd unit: $SYSTEMD_UNIT"
    rm -f "$SYSTEMD_UNIT"
    systemctl daemon-reload
fi

# Remove binary
if [[ -f "$BINARY_PATH" ]]; then
    log "Removing binary: $BINARY_PATH"
    rm -f "$BINARY_PATH"
fi

# Ask about config directory
if [[ -d "$CONFIG_DIR" ]]; then
    warn "Config directory exists: $CONFIG_DIR"
    warn "Contains: $(ls -1 "$CONFIG_DIR" | tr '\n' ', ' | sed 's/,$//')"
    read -p "Remove config directory (including secret key)? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log "Removing config directory: $CONFIG_DIR"
        rm -rf "$CONFIG_DIR"
    else
        log "Keeping config directory: $CONFIG_DIR"
    fi
fi

log "✓ Uninstallation complete."
