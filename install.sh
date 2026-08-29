#!/usr/bin/env bash
# Network System Monitor — Installer
#
# Usage (one-line):
#   curl -fsSL https://raw.githubusercontent.com/me60732/network-system-monitor/main/install.sh | sudo bash
#
# Note: use 'curl ... | sudo bash' NOT 'sudo curl ... | bash'
# The pipe always runs bash as a new process — sudo must be on bash, not curl.
#
# Or locally from the project root:
#   ./install.sh

set -euo pipefail

# ── Constants ─────────────────────────────────────────────────────────────
REPO="me60732/network-system-monitor"
GITHUB_RELEASES="https://github.com/${REPO}/releases/latest/download"
GITHUB_API="https://api.github.com/repos/${REPO}/releases/latest"
APP_ID="com.cosmic.network_system_monitor"
NMD_CONFIG_DIR="/etc/nmd"
NMD_CONFIG_FILE="${NMD_CONFIG_DIR}/config.toml"
NMD_SYSTEMD_UNIT="/etc/systemd/system/nmd-service.service"
NMD_HOME="/var/lib/nmd"
NMD_USER="nmd"

# ── Colors ────────────────────────────────────────────────────────────────
if [ -t 1 ]; then
  RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
  BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'
else
  RED=''; GREEN=''; YELLOW=''; BLUE=''; CYAN=''; BOLD=''; NC=''
fi

# ── Logging ───────────────────────────────────────────────────────────────
log()  { echo -e "  ${GREEN}✓${NC} $*"; }
info() { echo -e "  ${BLUE}→${NC} $*"; }
warn() { echo -e "  ${YELLOW}!${NC} $*"; }
err()  { echo -e "  ${RED}✗${NC} $*" >&2; }
die()  { err "$*"; exit 1; }
blank(){ echo ""; }

# ── Helpers ───────────────────────────────────────────────────────────────
has() { command -v "$1" &>/dev/null; }

need_root() {
  if [[ $EUID -ne 0 ]]; then
    err "This script must run as root."
    blank
    echo -e "  ${BOLD}Run with:${NC}"
    echo -e "  ${CYAN}curl -fsSL https://raw.githubusercontent.com/me60732/network-system-monitor/main/install.sh | sudo bash${NC}"
    blank
    echo -e "  ${YELLOW}Note:${NC} 'sudo curl ... | bash' only gives sudo to curl, not bash."
    echo -e "       The pipe runs bash as your normal user — use 'curl ... | sudo bash' instead."
    blank
    exit 1
  fi
}

download() {
  local url="$1" dest="$2"
  if has curl; then
    curl -fsSL "$url" -o "$dest"
  elif has wget; then
    wget -q "$url" -O "$dest"
  else
    die "Neither curl nor wget found — install one and retry."
  fi
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64)   echo "x86_64"  ;;
    aarch64|arm64)  echo "aarch64" ;;
    *)              echo "unsupported" ;;
  esac
}

get_latest_tag() {
  local tag=""
  if has curl; then
    tag=$(curl -fsSL "${GITHUB_API}" 2>/dev/null | grep '"tag_name"' | head -1 \
          | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
  elif has wget; then
    tag=$(wget -qO- "${GITHUB_API}" 2>/dev/null | grep '"tag_name"' | head -1 \
          | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
  fi
  echo "${tag:-unknown}"
}

# ── Header ────────────────────────────────────────────────────────────────
print_header() {
  blank
  echo -e "${BOLD}${BLUE}  ╔════════════════════════════════════════════╗"
  echo -e "  ║   Network System Monitor — Installer      ║"
  echo -e "  ╚════════════════════════════════════════════╝${NC}"
  blank
  echo -e "  ${CYAN}https://github.com/${REPO}${NC}"
  blank
}

# ── Component menu ────────────────────────────────────────────────────────
prompt_component() {
  blank
  echo -e "${BOLD}  What would you like to install?${NC}"
  blank
  echo -e "  ${GREEN}1)${NC} ${BOLD}Sender${NC}   ${YELLOW}nmd-service${NC}"
  echo -e "     Runs on remote Linux machines — collects and sends metrics"
  blank
  echo -e "  ${GREEN}2)${NC} ${BOLD}Receiver${NC} ${YELLOW}cosmic-applet${NC}"
  echo -e "     COSMIC desktop panel applet — displays local + remote metrics"
  echo -e "     ${CYAN}Requires: COSMIC desktop environment${NC}"
  blank
  echo -e "  ${GREEN}3)${NC} ${BOLD}Both${NC} (sender + receiver on this machine)"
  blank
  while true; do
    read -rp "  Choice [1-3]: " choice
    case "$choice" in
      1) echo "sender";   return ;;
      2) echo "receiver"; return ;;
      3) echo "both";     return ;;
      *) warn "Please enter 1, 2, or 3." ;;
    esac
  done
}

# ── Method menu ───────────────────────────────────────────────────────────
prompt_method() {
  local arch="$1"
  blank
  echo -e "${BOLD}  Install method?${NC}"
  blank
  echo -e "  ${GREEN}1)${NC} ${BOLD}Pre-built binary${NC} ${YELLOW}(${arch})${NC}"
  echo -e "     Downloads from GitHub releases — fastest option"
  blank
  echo -e "  ${GREEN}2)${NC} ${BOLD}Compile from source${NC}"
  echo -e "     Builds with cargo — installs Rust if needed (~10–20 min)"
  blank
  while true; do
    read -rp "  Choice [1-2]: " choice
    case "$choice" in
      1) echo "binary"; return ;;
      2) echo "source"; return ;;
      *) warn "Please enter 1 or 2." ;;
    esac
  done
}

# ── Rust installer ────────────────────────────────────────────────────────
ensure_rust() {
  if has cargo; then
    log "Rust/cargo already installed ($(cargo --version 2>/dev/null))"
    return
  fi
  info "Installing Rust via rustup..."
  local tmp
  tmp=$(mktemp)
  download "https://sh.rustup.rs" "$tmp"
  sh "$tmp" -y --default-toolchain stable --no-modify-path
  rm -f "$tmp"
  # shellcheck source=/dev/null
  source "${HOME}/.cargo/env" 2>/dev/null || export PATH="${HOME}/.cargo/bin:${PATH}"
  log "Rust installed ($(cargo --version 2>/dev/null))"
}

# ── System build deps ─────────────────────────────────────────────────────
install_build_deps() {
  local component="$1"
  if ! has apt-get; then
    warn "apt-get not found — install build dependencies manually."
    warn "See: https://github.com/${REPO}#building-from-source"
    return
  fi

  info "Installing build dependencies..."
  local pkgs="pkg-config libudev-dev"

  if [[ "$component" == "receiver" || "$component" == "both" ]]; then
    pkgs="$pkgs clang cmake libxkbcommon-dev libwayland-dev libdbus-1-dev \
          libinput-dev libgbm-dev libseat-dev libssl-dev libegl-dev \
          libpipewire-0.3-dev libfontconfig-dev"
  fi

  # shellcheck disable=SC2086
  apt-get update -qq && apt-get install -y --no-install-recommends $pkgs
  log "Build dependencies installed."
}

# ── Binary downloads ──────────────────────────────────────────────────────
download_and_extract() {
  local asset="$1" tmpdir="$2"
  local url="${GITHUB_RELEASES}/${asset}"
  info "Downloading ${asset}..."
  download "$url" "${tmpdir}/${asset}"
  tar -xzf "${tmpdir}/${asset}" -C "${tmpdir}"
}

install_sender_binary() {
  local arch="$1"
  local tmpdir
  tmpdir=$(mktemp -d)
  trap 'rm -rf "$tmpdir"' RETURN

  download_and_extract "nmd-service-${arch}-linux.tar.gz" "$tmpdir"
  install -m 755 "${tmpdir}/nmd-service" "/usr/local/bin/nmd-service"
  log "nmd-service installed to /usr/local/bin/nmd-service"
}

install_receiver_binary() {
  local arch="$1"
  local tmpdir
  tmpdir=$(mktemp -d)
  trap 'rm -rf "$tmpdir"' RETURN

  download_and_extract "cosmic-applet-${arch}-linux.tar.gz" "$tmpdir"
  _install_receiver_files "${tmpdir}/cosmic-applet" "${tmpdir}/res"
}

# ── Source builds ─────────────────────────────────────────────────────────
install_from_source() {
  local component="$1"
  has git || die "git is required for source installation. Install it and retry."

  ensure_rust
  install_build_deps "$component"

  local tmpdir
  tmpdir=$(mktemp -d)
  trap 'cd / && rm -rf "$tmpdir"' RETURN

  info "Cloning repository..."
  git clone --depth=1 "https://github.com/${REPO}.git" "${tmpdir}/repo"
  cd "${tmpdir}/repo"

  case "$component" in
    sender)
      info "Building nmd-service..."
      cargo build --release -p nmd-service
      install -m 755 target/release/nmd-service /usr/local/bin/nmd-service
      log "nmd-service built and installed."
      ;;
    receiver)
      info "Building cosmic-applet (this takes a while)..."
      cargo build --release -p cosmic-applet
      _install_receiver_files "target/release/cosmic-applet" "cosmic-applet/res"
      ;;
    both)
      info "Building nmd-service and cosmic-applet..."
      cargo build --release -p nmd-service -p cosmic-applet
      install -m 755 target/release/nmd-service /usr/local/bin/nmd-service
      log "nmd-service installed."
      _install_receiver_files "target/release/cosmic-applet" "cosmic-applet/res"
      ;;
  esac
}

# ── Shared receiver file install ──────────────────────────────────────────
_install_receiver_files() {
  local binary="$1" res_dir="$2"
  local INSTALL_BIN="/usr/local/bin"
  local INSTALL_DESKTOP="/usr/share/applications"
  local INSTALL_ICONS="/usr/share/icons/hicolor/scalable/apps"

  install -m 755 "$binary" "${INSTALL_BIN}/${APP_ID}"

  install -Dm644 "${res_dir}/${APP_ID}.desktop" "${INSTALL_DESKTOP}/${APP_ID}.desktop"
  sed -i "s|Exec=${APP_ID}|Exec=${INSTALL_BIN}/${APP_ID}|g" \
    "${INSTALL_DESKTOP}/${APP_ID}.desktop"

  mkdir -p "$INSTALL_ICONS"
  install -m 644 "${res_dir}/icons/${APP_ID}.svg" "${INSTALL_ICONS}/${APP_ID}.svg"

  gtk-update-icon-cache /usr/share/icons/hicolor/ 2>/dev/null || true
  update-desktop-database "$INSTALL_DESKTOP" 2>/dev/null || true

  log "cosmic-applet installed to ${INSTALL_BIN}/${APP_ID}"
}

# ── Sender: system user ───────────────────────────────────────────────────
ensure_nmd_user() {
  if ! id -u "$NMD_USER" &>/dev/null; then
    info "Creating system user '${NMD_USER}' for service isolation..."
    useradd --system \
            --create-home \
            --home-dir "$NMD_HOME" \
            --shell /usr/sbin/nologin \
            "$NMD_USER"
    log "User '${NMD_USER}' created."
  else
    log "System user '${NMD_USER}' already exists."
  fi

  # Ensure keypair dir is accessible to the service user
  mkdir -p "${NMD_HOME}/.config/nmd"
  chown -R "${NMD_USER}:${NMD_USER}" "$NMD_HOME"
}

# ── Sender: config ────────────────────────────────────────────────────────
setup_sender_config() {
  if [[ -f "$NMD_CONFIG_FILE" ]]; then
    warn "Config already exists at ${NMD_CONFIG_FILE} — leaving it unchanged."
    return
  fi

  blank
  echo -e "${BOLD}  Sender configuration${NC}"
  blank

  local default_id
  default_id=$(hostname -s 2>/dev/null || echo "my-machine")
  read -rp "  Machine name [${default_id}]: " machine_id
  machine_id="${machine_id:-$default_id}"

  read -rp "  Receiver IP address (desktop machine) [127.0.0.1]: " receiver_host
  receiver_host="${receiver_host:-127.0.0.1}"

  read -rp "  Receiver port [51057]: " receiver_port
  receiver_port="${receiver_port:-51057}"

  mkdir -p "$NMD_CONFIG_DIR"
  chown root:"$NMD_USER" "$NMD_CONFIG_DIR"
  chmod 750 "$NMD_CONFIG_DIR"

  cat > "$NMD_CONFIG_FILE" <<EOF
# nmd-service configuration — generated by install.sh on $(date)
# https://github.com/${REPO}

# Desktop machine running the cosmic-applet receiver
host = "${receiver_host}"
port = ${receiver_port}

# How often to collect and send metrics (seconds)
refresh_interval_secs = 1

# Unique name for this machine (shown in the applet UI)
machine_id = "${machine_id}"

# receiver_pubkey is configured automatically via TCP pairing when
# nmd-service first connects to the receiver. You can also set it
# manually if you prefer:
# receiver_pubkey = "<64-char hex X25519 public key from applet Settings>"
EOF

  chown root:"$NMD_USER" "$NMD_CONFIG_FILE"
  chmod 640 "$NMD_CONFIG_FILE"
  log "Config created: ${NMD_CONFIG_FILE}"
}

# ── Sender: systemd service ───────────────────────────────────────────────
install_systemd_service() {
  if ! has systemctl; then
    warn "systemd not found — skipping service installation."
    warn "Start manually: /usr/local/bin/nmd-service --config ${NMD_CONFIG_FILE}"
    return
  fi

  info "Installing systemd service..."
  cat > "$NMD_SYSTEMD_UNIT" <<EOF
[Unit]
Description=Network System Monitor — metrics sender
Documentation=https://github.com/${REPO}
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/nmd-service --config ${NMD_CONFIG_FILE}
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=nmd-service

# Run as dedicated unprivileged user
User=${NMD_USER}
Group=${NMD_USER}
Environment=HOME=${NMD_HOME}

# Security hardening
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
ReadWritePaths=${NMD_HOME}

[Install]
WantedBy=multi-user.target
EOF

  systemctl daemon-reload
  log "Systemd unit installed: ${NMD_SYSTEMD_UNIT}"

  blank
  read -rp "  Enable nmd-service to start on boot? [Y/n]: " enable_svc
  if [[ "${enable_svc:-Y}" =~ ^[Yy] ]]; then
    systemctl enable nmd-service
    log "Service enabled."
  fi

  read -rp "  Start nmd-service now? [Y/n]: " start_svc
  if [[ "${start_svc:-Y}" =~ ^[Yy] ]]; then
    systemctl start nmd-service
    sleep 2
    if systemctl is-active --quiet nmd-service; then
      log "Service is running."
    else
      warn "Service did not start — check: journalctl -u nmd-service -n 30"
    fi
  fi
}

# ── Post-install messages ─────────────────────────────────────────────────
print_sender_done() {
  blank
  echo -e "${BOLD}${GREEN}  ── Sender installed ──────────────────────────────────${NC}"
  blank
  echo -e "  ${CYAN}Config:${NC}   ${NMD_CONFIG_FILE}"
  echo -e "  ${CYAN}Logs:${NC}     journalctl -u nmd-service -f"
  echo -e "  ${CYAN}Restart:${NC}  systemctl restart nmd-service"
  blank
  echo -e "  ${BOLD}What happens next:${NC}"
  echo -e "  nmd-service will TCP-connect to the receiver and send a pairing"
  echo -e "  request. ${BOLD}Accept it in the COSMIC applet UI${NC} — metrics start"
  echo -e "  flowing within seconds of acceptance."
  blank
  echo -e "  If the receiver isn't running TCP pairing, copy the pubkey from"
  echo -e "  ${CYAN}Settings → General${NC} in the applet and set it in the config:"
  echo -e "  ${CYAN}receiver_pubkey = \"<hex>\"${NC}"
  blank
}

print_receiver_done() {
  blank
  echo -e "${BOLD}${GREEN}  ── Receiver installed ─────────────────────────────────${NC}"
  blank
  echo -e "  ${BOLD}To add the applet to your panel:${NC}"
  echo -e "  COSMIC Settings → Desktop → Panel → Add Applet"
  echo -e "  → Select ${CYAN}Network System Monitor${NC}"
  blank
  echo -e "  Local machine stats appear immediately."
  echo -e "  Remote machines appear after pairing (run installer on each)."
  blank
  echo -e "  ${BOLD}To restart the COSMIC panel:${NC}"
  echo -e "  ${CYAN}kill \$(pgrep -o cosmic-panel)${NC}"
  blank
}

# ── Main ──────────────────────────────────────────────────────────────────
main() {
  # When piped through bash (curl ... | sudo bash) stdin is the script itself,
  # not the terminal. Reopen stdin from /dev/tty so interactive prompts work.
  if [[ ! -t 0 ]]; then
    exec < /dev/tty || die "Cannot open /dev/tty for interactive input."
  fi

  print_header
  need_root

  local arch
  arch=$(detect_arch)
  if [[ "$arch" == "unsupported" ]]; then
    die "Unsupported architecture: $(uname -m). Use method 2 (compile from source) on this platform."
  fi

  info "Detected architecture: ${BOLD}${arch}${NC}"

  local component method
  component=$(prompt_component)
  method=$(prompt_method "$arch")

  blank
  info "Installing ${BOLD}${component}${NC} via ${BOLD}${method}${NC}..."
  blank

  # Warn about COSMIC requirement for receiver
  if [[ "$component" == "receiver" || "$component" == "both" ]]; then
    if ! has cosmic-panel && ! has cosmic-comp; then
      warn "COSMIC desktop environment not detected."
      warn "The applet requires COSMIC. See: https://system76.com/cosmic"
      read -rp "  Continue anyway? [y/N]: " cont
      [[ "${cont:-N}" =~ ^[Yy] ]] || { info "Cancelled."; exit 0; }
    fi
  fi

  # Install binaries
  if [[ "$method" == "binary" ]]; then
    case "$component" in
      sender)
        install_sender_binary "$arch"
        ;;
      receiver)
        install_receiver_binary "$arch"
        ;;
      both)
        install_sender_binary "$arch"
        install_receiver_binary "$arch"
        ;;
    esac
  else
    install_from_source "$component"
  fi

  # Post-install setup for sender
  if [[ "$component" == "sender" || "$component" == "both" ]]; then
    blank
    ensure_nmd_user
    setup_sender_config
    install_systemd_service
  fi

  # Done messages
  if [[ "$component" == "sender" || "$component" == "both" ]]; then
    print_sender_done
  fi
  if [[ "$component" == "receiver" || "$component" == "both" ]]; then
    print_receiver_done
  fi

  echo -e "${BOLD}${GREEN}  Installation complete.${NC}"
  blank
}

main "$@"
