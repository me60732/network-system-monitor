#!/usr/bin/env bash
# Network Monitor Daemon (nmd-service) Installation Script
# 
# One-command install for remote machines:
#   curl -fsSL https://raw.githubusercontent.com/USER/REPO/main/nmd-service/install-scripts/install.sh | bash
#
# Or manual:
#   ./install.sh

set -euo pipefail

# Configuration
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/nmd"
SECRET_KEY_FILE="$CONFIG_DIR/secret.key"
CONFIG_FILE="$CONFIG_DIR/config.toml"
SYSTEMD_UNIT="/etc/systemd/system/nmd-service.service"
SERVICE_NAME="nmd-service"
BINARY_NAME="nmd-service"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() { echo -e "${GREEN}[✓]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
error() { echo -e "${RED}[✗]${NC} $*" >&2; }
die() { error "$*"; exit 1; }

# Require root
[[ $EUID -eq 0 ]] || die "This script must be run as root (use sudo)"

# Detect desktop UDP receiver address (default: ask user)
read -p "Enter desktop machine IP address (where cosmic-applet runs): " DESKTOP_IP
[[ -n "$DESKTOP_IP" ]] || die "Desktop IP is required"

read -p "Enter desktop UDP port [51057]: " DESKTOP_PORT
DESKTOP_PORT="${DESKTOP_PORT:-51057}"

read -p "Enter this machine's name [$(hostname)]: " MACHINE_NAME
MACHINE_NAME="${MACHINE_NAME:-$(hostname)}"

log "Installing nmd-service for machine '$MACHINE_NAME' → $DESKTOP_IP:$DESKTOP_PORT"

# Check if binary exists in current directory or needs building
if [[ -f "./target/release/$BINARY_NAME" ]]; then
    BINARY_PATH="./target/release/$BINARY_NAME"
    log "Found release binary: $BINARY_PATH"
elif [[ -f "./$BINARY_NAME" ]]; then
    BINARY_PATH="./$BINARY_NAME"
    log "Found binary: $BINARY_PATH"
else
    warn "Binary not found. Building from source..."
    command -v cargo >/dev/null 2>&1 || die "cargo not found. Install Rust: https://rustup.rs"
    
    # Find workspace root (look for Cargo.toml with workspace definition)
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
    
    if [[ ! -f "$WORKSPACE_ROOT/Cargo.toml" ]]; then
        die "Cannot find workspace root. Run this script from the project directory."
    fi
    
    log "Building nmd-service..."
    cd "$WORKSPACE_ROOT"
    cargo build --release -p nmd-service
    BINARY_PATH="$WORKSPACE_ROOT/target/release/$BINARY_NAME"
fi

# Create dedicated system user for nmd service (SEC-02 fix)
if ! id -u nmd >/dev/null 2>&1; then
    log "Creating system user 'nmd' for service isolation"
    useradd --system --no-create-home --shell /usr/sbin/nologin nmd
fi

# Create config directory
log "Creating config directory: $CONFIG_DIR"
mkdir -p "$CONFIG_DIR"
chown root:nmd "$CONFIG_DIR"
chmod 750 "$CONFIG_DIR"

# Generate HMAC secret key (32 bytes raw — SEC-01 fix)
if [[ -f "$SECRET_KEY_FILE" ]]; then
    warn "Secret key already exists: $SECRET_KEY_FILE (keeping existing)"
    # Fix permissions on existing key
    chown root:nmd "$SECRET_KEY_FILE"
    chmod 640 "$SECRET_KEY_FILE"
else
    log "Generating HMAC secret key: $SECRET_KEY_FILE"
    # SEC-01: Generate 32 raw bytes (not hex-encoded)
    umask 077
    head -c 32 /dev/urandom > "$SECRET_KEY_FILE"
    chown root:nmd "$SECRET_KEY_FILE"
    chmod 640 "$SECRET_KEY_FILE"
    log "Secret key generated (32 raw bytes, root:nmd 640)"
    
    warn "IMPORTANT: Copy this key to the desktop machine:"
    warn "  Desktop path: $CONFIG_DIR/secret.key"
    warn "  Use: scp $SECRET_KEY_FILE user@desktop:~/.config/cosmic/network-monitor/secret.key"
    warn "  (Key is binary data — must use scp or similar for transfer)"
    echo ""
    read -p "Press Enter after copying the key to continue..."
fi

# Create config file (SEC-04: Fixed schema to match ServiceConfig)
log "Creating config file: $CONFIG_FILE"
cat > "$CONFIG_FILE" <<EOF
# Network Monitor Daemon Configuration
# Machine: $MACHINE_NAME
# Generated: $(date)

# Desktop UDP receiver address (top-level fields per ServiceConfig)
host = "$DESKTOP_IP"
port = $DESKTOP_PORT

# Metrics push interval in seconds (default: 1)
refresh_interval_secs = 2

# This machine's identifier (must be unique across your network)
machine_id = "$MACHINE_NAME"

# HMAC secret key path (must match desktop)
hmac_secret_path = "$SECRET_KEY_FILE"
EOF

chown root:nmd "$CONFIG_FILE"
chmod 640 "$CONFIG_FILE"
log "Config created: $CONFIG_FILE (root:nmd 640)"

# Install binary
log "Installing binary: $BINARY_PATH → $INSTALL_DIR/$BINARY_NAME"
cp "$BINARY_PATH" "$INSTALL_DIR/$BINARY_NAME"
chmod 755 "$INSTALL_DIR/$BINARY_NAME"

# Create systemd unit file
log "Creating systemd unit: $SYSTEMD_UNIT"
cat > "$SYSTEMD_UNIT" <<EOF
[Unit]
Description=Network Monitor Daemon
Documentation=https://github.com/USER/REPO
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=$INSTALL_DIR/$BINARY_NAME --config $CONFIG_FILE
Restart=always
RestartSec=5

# Security hardening (SEC-02: Run as dedicated nmd user)
User=nmd
Group=nmd
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ProtectKernelTunables=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictSUIDSGID=true
ProtectKernelModules=true
ProtectKernelLogs=true
RestrictNamespaces=true

[Install]
WantedBy=multi-user.target
EOF

# Reload systemd and enable service
log "Reloading systemd daemon"
systemctl daemon-reload

log "Enabling $SERVICE_NAME to start on boot"
systemctl enable "$SERVICE_NAME"

log "Starting $SERVICE_NAME"
systemctl start "$SERVICE_NAME"

# Wait for startup
sleep 2

# Check status
if systemctl is-active --quiet "$SERVICE_NAME"; then
    log "✓ Installation complete! Service is running."
    log ""
    log "Check status:  systemctl status $SERVICE_NAME"
    log "View logs:     journalctl -u $SERVICE_NAME -f"
    log "Config file:   $CONFIG_FILE"
    log "Secret key:    $SECRET_KEY_FILE"
else
    error "Service failed to start. Check logs:"
    error "  journalctl -u $SERVICE_NAME -n 50"
    exit 1
fi
