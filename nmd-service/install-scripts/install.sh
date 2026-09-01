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
CONFIG_FILE="$CONFIG_DIR/config.toml"
SYSTEMD_UNIT="/etc/systemd/system/nmd.service"
SERVICE_NAME="nmd"
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

# Create dedicated system user for nmd service with home directory
if ! id -u nmd >/dev/null 2>&1; then
    log "Creating system user 'nmd' for service isolation"
    useradd --system --create-home --home-dir /var/lib/nmd --shell /usr/sbin/nologin nmd
fi

# Create config directory
log "Creating config directory: $CONFIG_DIR"
mkdir -p "$CONFIG_DIR"
chown root:nmd "$CONFIG_DIR"
chmod 750 "$CONFIG_DIR"

# Create keypair directory for the nmd user
log "Creating keypair directory: /var/lib/nmd/.config/nmd"
mkdir -p /var/lib/nmd/.config/nmd
chown nmd:nmd /var/lib/nmd
chown nmd:nmd /var/lib/nmd/.config
chown nmd:nmd /var/lib/nmd/.config/nmd

# Create config file (per ServiceConfig with #[serde(deny_unknown_fields)])
log "Creating config file: $CONFIG_FILE"
cat > "$CONFIG_FILE" <<EOF
# Network Monitor Daemon Configuration
# Machine: $MACHINE_NAME
# Generated: $(date)

# Desktop UDP receiver address
host = "$DESKTOP_IP"
port = $DESKTOP_PORT

# Metrics push interval in seconds
refresh_interval_secs = 1

# Unique machine identifier (must be unique across your network)
machine_id = "$MACHINE_NAME"

# receiver_pubkey is set here after pairing is accepted in the applet UI.
# Leave absent for initial bootstrap — a pairing request will appear in the applet.
# receiver_pubkey = "paste-hex-pubkey-from-applet-settings-here"
EOF

chown root:nmd "$CONFIG_FILE"
chmod 660 "$CONFIG_FILE"
log "Config created: $CONFIG_FILE (root:nmd 660)"

# Install binary
log "Installing binary: $BINARY_PATH → $INSTALL_DIR/$BINARY_NAME"
cp "$BINARY_PATH" "$INSTALL_DIR/$BINARY_NAME"
chmod 755 "$INSTALL_DIR/$BINARY_NAME"

# Create systemd unit file with HOME environment and keypair directory access
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

# Security hardening (run as dedicated nmd user with home directory)
User=nmd
Group=nmd
Environment="HOME=/var/lib/nmd"
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ProtectKernelTunables=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictSUIDSGID=true
ProtectKernelModules=true
ProtectKernelLogs=true
RestrictNamespaces=true
ReadWritePaths=/var/lib/nmd /etc/nmd

[Install]
WantedBy=multi-user.target
EOF

# Reload systemd and enable service
log "Reloading systemd daemon"
systemctl daemon-reload

log "Enabling nmd to start on boot"
systemctl enable nmd

log "Starting nmd"
systemctl start nmd

# Wait for startup
sleep 2

# Check status
if systemctl is-active --quiet nmd; then
    log "✓ Installation complete! Service is running."
    log ""
    log "Check status:  systemctl status nmd"
    log "View logs:     journalctl -u $SERVICE_NAME -f"
    log "Config file:   $CONFIG_FILE"
    log "Keypair:       /var/lib/nmd/.config/nmd/keypair.key (auto-generated on first start)"
    log ""
    log "NEXT STEP: nmd-service will automatically TCP-connect to the receiver and"
    log "request pairing. Accept it in the cosmic-applet UI — done!"
    log ""
    log "If TCP pairing fails, manually copy the pubkey from Settings → General"
    log "in the applet and set: receiver_pubkey = \"<hex>\" in $CONFIG_FILE"
    log "Then restart: systemctl restart $SERVICE_NAME"
else
    error "Service failed to start. Check logs:"
    error "  journalctl -u $SERVICE_NAME -n 50"
    exit 1
fi
