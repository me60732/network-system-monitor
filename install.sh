#!/usr/bin/env bash
# Network System Monitor — Installer
#
# Usage (one-line):
#   curl -fsSL https://raw.githubusercontent.com/me60732/network-system-monitor/main/install.sh | sudo bash
#
# To update what is already on this machine, or remove it again, pass a mode after 'bash -s --':
#   curl -fsSL https://raw.githubusercontent.com/me60732/network-system-monitor/main/install.sh | sudo bash -s -- --update
#   curl -fsSL https://raw.githubusercontent.com/me60732/network-system-monitor/main/install.sh | sudo bash -s -- --uninstall
#
# Note: use 'curl ... | sudo bash' NOT 'sudo curl ... | bash'
# The pipe always runs bash as a new process — sudo must be on bash, not curl.
#
# Or locally from the project root:
#   sudo ./install.sh              # install (same as no argument)
#   sudo ./install.sh --update     # replace binaries in place, keep config
#   sudo ./install.sh --uninstall  # remove what this script installs
#   sudo ./install.sh --help       # modes and options

set -euo pipefail

# ── Constants ─────────────────────────────────────────────────────────────
REPO="me60732/network-system-monitor"
GITHUB_RELEASES="https://github.com/${REPO}/releases/latest/download"
GITHUB_API="https://api.github.com/repos/${REPO}/releases/latest"
RAW_INSTALL_URL="https://raw.githubusercontent.com/${REPO}/main/install.sh"
APP_ID="com.cosmic.network_system_monitor"
NMD_CONFIG_DIR="/etc/nmd"
NMD_CONFIG_FILE="${NMD_CONFIG_DIR}/config.toml"
NMD_SYSTEMD_UNIT="/etc/systemd/system/nmd-service.service"
NMD_HOME="/var/lib/nmd"
NMD_USER="nmd"

# Install destinations — shared by the install, update and uninstall paths.
INSTALL_BIN="/usr/local/bin"
INSTALL_DESKTOP="/usr/share/applications"
INSTALL_ICONS="/usr/share/icons/hicolor/scalable/apps"
INSTALL_DOC="/usr/share/doc"
SENDER_BIN="${INSTALL_BIN}/nmd-service"
RECEIVER_BIN="${INSTALL_BIN}/${APP_ID}"

# The app has shipped under a second set of paths, and an install may have been done by either
# installer, so update and uninstall both have to account for each of these:
#  - cosmic-applet/install-local.sh installs the receiver as /usr/local/bin/cosmic-applet while
#    this script installs it as /usr/local/bin/<app-id>; whichever ran last owns the desktop
#    Exec= line and the other copy stays behind unreferenced.
#  - nmd-service/install-scripts/install.sh writes the unit as nmd.service (service name 'nmd'),
#    this script writes nmd-service.service.
RECEIVER_BIN_ALT="${INSTALL_BIN}/cosmic-applet"
NMD_SYSTEMD_UNIT_ALT="/etc/systemd/system/nmd.service"

# Per-user receiver state (config + TOFU pairing keys + receiver identity). Relative to the
# invoking user's home, because the installer only ever writes system-wide paths.
USER_STATE_RECEIVER=".config/cosmic-applet"

# ── Colors (only when stdout is a real terminal) ──────────────────────────
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

# Temporary directories, removed once by the EXIT trap. A `trap ... RETURN` set inside a function
# stays armed for the enclosing function's return too, so referencing a function-local there fails
# with "unbound variable" under set -u — which made a successful install exit non-zero.
CLEANUP_DIRS=()
cleanup_dirs() {
  local dir
  if [[ ${#CLEANUP_DIRS[@]} -gt 0 ]]; then
    for dir in "${CLEANUP_DIRS[@]}"; do
      if [[ -n "$dir" && -d "$dir" ]]; then rm -rf -- "$dir"; fi
    done
  fi
  return 0
}
trap cleanup_dirs EXIT

list_contains() {
  local needle="$1" item
  shift
  for item in "$@"; do
    if [[ "$item" == "$needle" ]]; then return 0; fi
  done
  return 1
}

need_root() {
  if [[ $EUID -ne 0 ]]; then
    err "This script must run as root."
    blank
    echo -e "  ${BOLD}Run with:${NC}"
    echo -e "  ${CYAN}$(one_line_cmd)${NC}"
    blank
    echo -e "  ${YELLOW}Note:${NC} 'sudo curl ... | bash' only gives sudo to curl, not bash."
    echo -e "       Use 'curl ... | sudo bash' so the script itself runs as root."
    blank
    exit 1
  fi
}

# The copy-paste invocation for the current mode. Arguments for a piped script go after 'bash -s --'.
one_line_cmd() {
  local suffix=""
  case "$MODE" in
    update)    suffix=" -s -- --update"    ;;
    uninstall) suffix=" -s -- --uninstall" ;;
  esac
  echo "curl -fsSL ${RAW_INSTALL_URL} | sudo bash${suffix}"
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
    x86_64|amd64)  echo "x86_64"  ;;
    aarch64|arm64) echo "aarch64" ;;
    *)             echo "unsupported" ;;
  esac
}

# Default to yes/no on the prompt; reads from the tty because this script is commonly piped
# (curl … | sudo bash), which occupies stdin.
confirm() {
  local prompt="$1" def="${2:-N}" reply
  read -rp "  ${prompt} [${def}/n]: " reply </dev/tty || reply="$def"
  [[ "${reply:-$def}" =~ ^[Yy] ]]
}

# ── What is already on this machine ───────────────────────────────────────
# Both components are detected from the filesystem rather than from a manifest the installer
# writes, so an install done by any of this repo's three scripts is recognised.
sender_installed() {
  [[ -e "$SENDER_BIN" || -e "$NMD_SYSTEMD_UNIT" || -e "$NMD_SYSTEMD_UNIT_ALT" || -d "$NMD_CONFIG_DIR" ]]
}

receiver_installed() {
  [[ -e "$RECEIVER_BIN" || -e "$RECEIVER_BIN_ALT" || -e "${INSTALL_DESKTOP}/${APP_ID}.desktop" ]]
}

# Unit names the sender may be installed under, as "name:unit-file".
SENDER_UNITS=("nmd-service:${NMD_SYSTEMD_UNIT}" "nmd:${NMD_SYSTEMD_UNIT_ALT}")

# Sender version, or empty. Only ever run against nmd-service — it is the one binary with a
# --version flag; cosmic-applet takes no arguments and would start the applet instead.
sender_version() {
  local out=""
  if [[ -x "$SENDER_BIN" ]]; then
    out=$("$SENDER_BIN" --version 2>/dev/null | head -n1) || out=""
  fi
  printf '%s' "$out"
}

# Best-effort latest release tag (e.g. v0.1.3); empty when the API is unreachable.
latest_release_tag() {
  local tmp tag=""
  tmp=$(mktemp)
  if download "$GITHUB_API" "$tmp" 2>/dev/null; then
    tag=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$tmp" | head -n1)
  fi
  rm -f "$tmp"
  printf '%s' "$tag"
}

# Home directory of whoever invoked sudo, since the receiver keeps per-user state and root's own
# home does not contain it. Falls back to $HOME when the lookup yields nothing.
state_user_home() {
  local user="${SUDO_USER:-}" home=""
  if [[ -n "$user" && "$user" != "root" ]] && has getent; then
    home=$(getent passwd "$user" 2>/dev/null | cut -d: -f6)
  fi
  if [[ -z "$home" ]]; then
    home="${HOME:-/root}"
  fi
  printf '%s' "$home"
}

# ── Modes ─────────────────────────────────────────────────────────────────
MODE="install"

usage() {
  cat <<EOF
Network System Monitor — installer

Usage: install.sh [MODE]

  (no mode)     Same as --install.
  --install     Install the sender, the receiver, or both (asks which).
  --update      Replace the files of whatever is already on this machine with the latest
                release, keeping config, the nmd system user and the pairing keys.
  --uninstall   Remove the installed components, offering to delete config and state too.
  -h, --help    Show this help.

Must run as root. For a piped run, mode flags go after 'bash -s --':

  ${RAW_INSTALL_URL} | sudo bash
  ${RAW_INSTALL_URL} | sudo bash -s -- --update
  ${RAW_INSTALL_URL} | sudo bash -s -- --uninstall
EOF
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --install)            MODE="install"   ;;
      --update|-u)          MODE="update"    ;;
      --uninstall|--remove) MODE="uninstall" ;;
      -h|--help)            usage; exit 0 ;;
      *)
        err "Unknown option: $1"
        blank
        usage >&2
        exit 1
        ;;
    esac
    shift
  done
}

# ── Header ────────────────────────────────────────────────────────────────
print_header() {
  local title
  case "$MODE" in
    install)   title="Installer"   ;;
    update)    title="Updater"     ;;
    uninstall) title="Uninstaller" ;;
  esac
  blank
  echo -e "${BOLD}${BLUE}  ╔════════════════════════════════════════════╗"
  echo -e "  ║   Network System Monitor — ${title}$(printf '%*s' $(( 16 - ${#title} )))║${NC}"
  echo -e "  ╚════════════════════════════════════════════╝${NC}"
  blank
  echo -e "  ${CYAN}https://github.com/${REPO}${NC}"
  blank
}

# ── Component selection ───────────────────────────────────────────────────
# NOTE: called directly (not via $()) so echo output goes to the terminal.
# Result stored in global SELECTED_COMPONENT.
SELECTED_COMPONENT=""
prompt_component() {
  blank
  echo -e "${BOLD}  What would you like to install?${NC}"
  blank
  echo -e "  ${GREEN}1)${NC} ${BOLD}Sender${NC}   ${YELLOW}(nmd-service)${NC}"
  echo -e "     Runs on remote Linux machines — collects and sends metrics"
  blank
  echo -e "  ${GREEN}2)${NC} ${BOLD}Receiver${NC} ${YELLOW}(cosmic-applet)${NC}"
  echo -e "     COSMIC desktop panel applet — displays local + remote machine metrics"
  echo -e "     ${CYAN}Requires: COSMIC desktop environment${NC}"
  blank
  echo -e "  ${GREEN}3)${NC} ${BOLD}Both${NC} — sender and receiver on this machine"
  blank
  while true; do
    read -rp "  Choice [1-3]: " choice </dev/tty
    case "$choice" in
      1) SELECTED_COMPONENT="sender";   return ;;
      2) SELECTED_COMPONENT="receiver"; return ;;
      3) SELECTED_COMPONENT="both";     return ;;
      *) warn "Please enter 1, 2, or 3." ;;
    esac
  done
}

# ── Install method selection ──────────────────────────────────────────────
# NOTE: called directly (not via $()).
# Result stored in global SELECTED_METHOD.
SELECTED_METHOD=""
prompt_method() {
  local arch="$1"
  blank
  echo -e "${BOLD}  Install method?${NC}"
  blank
  echo -e "  ${GREEN}1)${NC} ${BOLD}Pre-built binary${NC} ${YELLOW}(${arch})${NC}"
  echo -e "     Downloads from GitHub releases — fastest"
  blank
  echo -e "  ${GREEN}2)${NC} ${BOLD}Compile from source${NC}"
  echo -e "     Builds with cargo — installs Rust if needed (~10–20 min first build)"
  blank
  while true; do
    read -rp "  Choice [1-2]: " choice </dev/tty
    case "$choice" in
      1) SELECTED_METHOD="binary"; return ;;
      2) SELECTED_METHOD="source"; return ;;
      *) warn "Please enter 1 or 2." ;;
    esac
  done
}

# ── Rust ──────────────────────────────────────────────────────────────────
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

# ── Build dependencies ────────────────────────────────────────────────────
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
_extract() {
  local asset="$1" tmpdir="$2"
  info "Downloading ${asset}..."
  download "${GITHUB_RELEASES}/${asset}" "${tmpdir}/${asset}"
  tar -xzf "${tmpdir}/${asset}" -C "${tmpdir}"
}

# GPLv3 s4/s6: conveying a binary requires conveying a copy of its license with it.
# Guarded because releases published before this change carry no LICENSE inside, and
# `set -e` would otherwise abort the install on a missing file.
_convey_license() {
  local src="$1" doc_name="$2" doc_root="${3:-$INSTALL_DOC}"
  if [ -f "$src" ]; then
    install -Dm644 "$src" "${doc_root}/${doc_name}/LICENSE"
  else
    warn "No LICENSE found next to the ${doc_name} binary."
    warn "  The binary is still GPL-3.0-only; fetch the license from the repository:"
    warn "  https://github.com/${REPO}"
  fi
}

install_sender_binary() {
  local arch="$1"
  local tmpdir; tmpdir=$(mktemp -d)
  CLEANUP_DIRS+=("$tmpdir")
  _extract "nmd-service-${arch}-linux.tar.gz" "$tmpdir"
  install -m 755 "${tmpdir}/nmd-service" "$SENDER_BIN"
  _convey_license "${tmpdir}/LICENSE" "nmd-service"
  log "nmd-service installed to ${SENDER_BIN}"
}

install_receiver_binary() {
  local arch="$1"
  local tmpdir; tmpdir=$(mktemp -d)
  CLEANUP_DIRS+=("$tmpdir")
  _extract "cosmic-applet-${arch}-linux.tar.gz" "$tmpdir"
  _install_receiver_files "${tmpdir}/cosmic-applet" "${tmpdir}/res" "${tmpdir}/LICENSE"
}

# ── Source builds ─────────────────────────────────────────────────────────
install_from_source() {
  local component="$1"
  has git || die "git is required for source installation. Install it and retry."
  ensure_rust
  install_build_deps "$component"

  local tmpdir; tmpdir=$(mktemp -d)
  CLEANUP_DIRS+=("$tmpdir")

  info "Cloning repository..."
  git clone --depth=1 "https://github.com/${REPO}.git" "${tmpdir}/repo"
  cd "${tmpdir}/repo"

  case "$component" in
    sender)
      info "Building nmd-service..."
      cargo build --release -p nmd-service
      install -m 755 target/release/nmd-service "$SENDER_BIN"
      _convey_license "LICENSE" "nmd-service"
      log "nmd-service built and installed."
      ;;
    receiver)
      info "Building cosmic-applet (this takes a while on first build)..."
      cargo build --release -p cosmic-applet
      _install_receiver_files "target/release/cosmic-applet" "cosmic-applet/res" "LICENSE"
      ;;
    both)
      info "Building nmd-service and cosmic-applet..."
      cargo build --release -p nmd-service -p cosmic-applet
      install -m 755 target/release/nmd-service "$SENDER_BIN"
      _convey_license "LICENSE" "nmd-service"
      log "nmd-service installed."
      _install_receiver_files "target/release/cosmic-applet" "cosmic-applet/res" "LICENSE"
      ;;
  esac

  # Leave the clone behind before its directory is deleted by the EXIT trap.
  cd /
}

# ── Shared receiver file installation ────────────────────────────────────
_install_receiver_files() {
  local binary="$1" res_dir="$2" license_src="$3"

  install -m 755 "$binary" "${INSTALL_BIN}/${APP_ID}"

  install -Dm644 "${res_dir}/${APP_ID}.desktop" "${INSTALL_DESKTOP}/${APP_ID}.desktop"
  sed -i "s|Exec=${APP_ID}|Exec=${INSTALL_BIN}/${APP_ID}|g" \
    "${INSTALL_DESKTOP}/${APP_ID}.desktop"

  mkdir -p "$INSTALL_ICONS"
  # The panel resolves sensor icons by freedesktop icon name, so every SVG in res/icons/
  # has to be installed — the app icon plus the per-sensor icons panel_widget.rs looks up.
  # Installing only the app icon leaves the panel sensors resolving against another
  # applet's exported icons and disappearing when that applet is removed.
  for svg in "${res_dir}"/icons/*.svg; do
    install -m 644 "$svg" "${INSTALL_ICONS}/$(basename "$svg")"
  done

  gtk-update-icon-cache /usr/share/icons/hicolor/ 2>/dev/null || true
  update-desktop-database "$INSTALL_DESKTOP" 2>/dev/null || true

  # GPLv3 s4/s6: conveying the binary requires conveying a copy of the license.
  _convey_license "$license_src" "$APP_ID"

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

  read -rp "  Machine name [${default_id}]: " machine_id </dev/tty
  machine_id="${machine_id:-$default_id}"

  read -rp "  Receiver IP address (desktop running the applet) [127.0.0.1]: " receiver_host </dev/tty
  receiver_host="${receiver_host:-127.0.0.1}"

  read -rp "  Receiver port [51057]: " receiver_port </dev/tty
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

# receiver_pubkey is set automatically via TCP pairing on first start.
# You can also set it manually if TCP pairing is unavailable:
# receiver_pubkey = "<64-char hex X25519 public key from applet Settings → General>"
EOF

  chown root:"$NMD_USER" "$NMD_CONFIG_FILE"
  chmod 640 "$NMD_CONFIG_FILE"
  log "Config created: ${NMD_CONFIG_FILE}"
}

# ── Sender: systemd service ───────────────────────────────────────────────
install_systemd_service() {
  if ! has systemctl; then
    warn "systemd not found — skipping service installation."
    warn "Start manually: ${SENDER_BIN} --config ${NMD_CONFIG_FILE}"
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
  read -rp "  Enable nmd-service to start on boot? [Y/n]: " enable_svc </dev/tty
  if [[ "${enable_svc:-Y}" =~ ^[Yy] ]]; then
    systemctl enable nmd-service
    log "Service enabled."
  fi

  read -rp "  Start nmd-service now? [Y/n]: " start_svc </dev/tty
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
  echo -e "  ${BOLD}To restart the COSMIC panel after install:${NC}"
  echo -e "  ${CYAN}kill \$(pgrep -o cosmic-panel)${NC}"
  blank
}

# ── Update ────────────────────────────────────────────────────────────────
# An update replaces the files this installer owns and nothing else: the sender's config, the
# nmd system user, the receiver's pairing keys and the per-user applet config are all left
# exactly as they were found.
sender_service_active() {
  local entry name
  for entry in "${SENDER_UNITS[@]}"; do
    name="${entry%%:*}"
    if systemctl is-active --quiet "$name" 2>/dev/null; then
      printf '%s' "$name"
      return 0
    fi
  done
}

# active_unit is whatever was running before the swap; "" means the service was not running and
# stays not running.
restore_sender_service() {
  local active_unit="$1"

  # An update leaves ownership alone; the account only has to exist for the unit to run.
  if ! id -u "$NMD_USER" >/dev/null 2>&1; then
    warn "System user '${NMD_USER}' is missing — recreating it."
    ensure_nmd_user
  fi

  if [[ ! -f "$NMD_CONFIG_FILE" ]]; then
    warn "No config at ${NMD_CONFIG_FILE} — an update never overwrites one, but this machine has none."
    if confirm "Create it now?" Y; then
      setup_sender_config
    fi
  else
    log "Kept existing config: ${NMD_CONFIG_FILE}"
    info "Options added since the last release are listed in CONFIGURATION.md; none are written automatically."
  fi

  if ! has systemctl; then
    warn "systemd not found — start the new binary yourself:"
    warn "  ${SENDER_BIN} --config ${NMD_CONFIG_FILE}"
    return
  fi

  systemctl daemon-reload
  if [[ -n "$active_unit" ]]; then
    systemctl start "$active_unit"
    sleep 2
    if systemctl is-active --quiet "$active_unit"; then
      log "${active_unit} restarted."
    else
      warn "${active_unit} did not start — check: journalctl -u ${active_unit} -n 30"
    fi
  else
    info "Service was not running before the update, so it was left stopped."
    info "Start it with: systemctl start nmd-service"
  fi
}

run_update() {
  local components=()
  if sender_installed;   then components+=("sender");   fi
  if receiver_installed; then components+=("receiver"); fi

  if [[ ${#components[@]} -eq 0 ]]; then
    die "Nothing to update — neither the sender nor the receiver is installed here. Run install.sh without --update to install."
  fi

  local arch
  arch=$(detect_arch)
  info "Installed: ${BOLD}${components[*]}${NC}"

  local latest
  latest=$(latest_release_tag)
  if [[ -n "$latest" ]]; then
    info "Latest release: ${BOLD}${latest}${NC}"
  else
    warn "Could not read the latest release tag from GitHub — continuing regardless."
  fi
  if list_contains "sender" "${components[@]}"; then
    local current
    current=$(sender_version)
    if [[ -n "$current" ]]; then
      info "Installed sender reports: ${BOLD}${current}${NC}"
    fi
  fi

  local method method_label
  if [[ "$arch" == "unsupported" ]]; then
    # There is no release asset for this architecture, so a source build is the only option.
    method="source"
    method_label="source build"
    info "No pre-built binary for $(uname -m) — compiling from source."
  else
    prompt_method "$arch"
    method="$SELECTED_METHOD"
    method_label="pre-built binary"
    if [[ "$method" == "source" ]]; then method_label="source build"; fi
  fi

  blank
  if ! confirm "Update ${components[*]} via ${method_label}?" Y; then
    info "Cancelled."
    exit 0
  fi

  # Stop the daemon before swapping the binary under it, so nothing keeps running the old code
  # with files changing underneath it.
  local active_unit=""
  if list_contains "sender" "${components[@]}" && has systemctl; then
    active_unit=$(sender_service_active)
    if [[ -n "$active_unit" ]]; then
      info "Stopping ${active_unit}..."
      systemctl stop "$active_unit"
    fi
  fi

  blank
  if [[ "$method" == "binary" ]]; then
    local component
    for component in "${components[@]}"; do
      case "$component" in
        sender)   install_sender_binary   "$arch" ;;
        receiver) install_receiver_binary "$arch" ;;
      esac
    done
  else
    local target="${components[0]}"
    if [[ ${#components[@]} -eq 2 ]]; then target="both"; fi
    install_from_source "$target"
  fi

  if list_contains "sender" "${components[@]}"; then
    restore_sender_service "$active_unit"
  fi

  if list_contains "receiver" "${components[@]}"; then
    blank
    echo -e "  ${BOLD}Restart the COSMIC panel to load the new applet:${NC}"
    echo -e "  ${CYAN}kill \$(pgrep -o cosmic-panel)${NC}"
    if [[ -e "$RECEIVER_BIN_ALT" ]]; then
      warn "${RECEIVER_BIN_ALT} was left in place — the desktop file now launches ${RECEIVER_BIN}."
      warn "  Run --uninstall and reinstall if you want a single copy."
    fi
  fi

  blank
  echo -e "${BOLD}${GREEN}  Update complete.${NC}"
  local after
  after=$(sender_version)
  if list_contains "sender" "${components[@]}" && [[ -n "$after" ]]; then
    echo -e "  ${CYAN}nmd-service now reports:${NC} ${after}"
  fi
  blank
}

# ── Uninstall ─────────────────────────────────────────────────────────────
_rm() {
  local path="$1"
  if [[ -L "$path" || -e "$path" ]]; then
    rm -f -- "$path"
    log "Removed ${path}"
  fi
}

# Refuses anything that is not a real directory path, because this runs as root and the callers
# build their arguments from a resolved home directory.
_rm_tree() {
  local path="$1"
  case "$path" in
    ""|/|//) err "Refusing to remove '${path}'"; return 1 ;;
  esac
  if [[ "${path#/}" == "$path" ]]; then
    err "Refusing to remove relative path '${path}'"
    return 1
  fi
  if [[ -d "$path" ]]; then
    rm -rf -- "$path"
    log "Removed ${path}/"
  fi
}

uninstall_sender() {
  blank
  info "Removing sender (nmd-service)..."

  if has systemctl; then
    local entry name path
    for entry in "${SENDER_UNITS[@]}"; do
      name="${entry%%:*}"
      path="${entry#*:}"
      if systemctl is-active --quiet "$name" 2>/dev/null; then
        systemctl stop "$name"
        log "Stopped ${name}."
      fi
      if systemctl is-enabled --quiet "$name" 2>/dev/null; then
        systemctl disable "$name"
        log "Disabled ${name}."
      fi
      _rm "$path"
    done
    systemctl daemon-reload
    systemctl reset-failed >/dev/null 2>&1 || true
    log "systemd reloaded."
  fi

  _rm "$SENDER_BIN"
  _rm "${INSTALL_DOC}/nmd-service/LICENSE"
  rmdir "${INSTALL_DOC}/nmd-service" 2>/dev/null || true

  if [[ -d "$NMD_CONFIG_DIR" ]]; then
    warn "Config directory ${NMD_CONFIG_DIR} holds: $(ls -1 "$NMD_CONFIG_DIR" 2>/dev/null | tr '\n' ' ')"
    if confirm "Delete ${NMD_CONFIG_DIR}?" N; then
      _rm_tree "$NMD_CONFIG_DIR"
    else
      info "Kept ${NMD_CONFIG_DIR}."
    fi
  fi

  if id -u "$NMD_USER" >/dev/null 2>&1; then
    warn "System user '${NMD_USER}' owns ${NMD_HOME}/.config/nmd — the sender's identity keypair"
    warn "  and the receiver pubkey it paired with. Deleting the user breaks re-pairing."
    if confirm "Delete user '${NMD_USER}' and ${NMD_HOME}?" N; then
      if has userdel; then
        userdel "$NMD_USER" 2>/dev/null || warn "userdel '${NMD_USER}' failed — remove the account manually."
      else
        warn "userdel not found — the account itself was left in place."
      fi
      _rm_tree "$NMD_HOME"
    else
      info "Kept user '${NMD_USER}' and ${NMD_HOME}."
    fi
  fi
}

uninstall_receiver() {
  blank
  info "Removing receiver (cosmic-applet)..."

  # Both names the applet ships under: this script installs the app id, cosmic-applet/install-local.sh
  # installs the bare binary name. Only files named after this app are touched, so icons belonging
  # to other applets in the same directory stay put.
  _rm "$RECEIVER_BIN"
  _rm "$RECEIVER_BIN_ALT"
  _rm "${INSTALL_DESKTOP}/${APP_ID}.desktop"
  _rm "${INSTALL_DOC}/${APP_ID}/LICENSE"
  rmdir "${INSTALL_DOC}/${APP_ID}" 2>/dev/null || true

  local svg
  for svg in "${INSTALL_ICONS}/${APP_ID}"*.svg; do
    if [[ -e "$svg" ]]; then _rm "$svg"; fi
  done

  gtk-update-icon-cache /usr/share/icons/hicolor/ 2>/dev/null || true
  update-desktop-database "$INSTALL_DESKTOP" 2>/dev/null || true
  log "Icon and desktop caches refreshed."

  local home state
  home=$(state_user_home)
  state="${home}/${USER_STATE_RECEIVER}"
  if [[ -d "$state" ]]; then
    warn "Per-user state ${state} holds config.toml, receiver.key and pairing.toml."
    warn "  pairing.toml is the list of machines allowed to talk to this desktop — deleting it"
    warn "  means every remote machine has to pair again."
    if confirm "Delete ${state}?" N; then
      _rm_tree "$state"
    else
      info "Kept ${state}."
    fi
  fi

  blank
  echo -e "  ${BOLD}If the panel is still running, restart it to drop the removed applet:${NC}"
  echo -e "  ${CYAN}kill \$(pgrep -o cosmic-panel)${NC}"
  blank
  echo -e "  ${BOLD}Also remove the applet from the panel config${NC} (COSMIC Settings → Desktop → Panel)"
  echo -e "  so it is not offered again after a reinstall."
}

run_uninstall() {
  local have_sender=0 have_receiver=0
  if sender_installed;   then have_sender=1;   fi
  if receiver_installed; then have_receiver=1; fi

  if [[ $have_sender -eq 0 && $have_receiver -eq 0 ]]; then
    warn "Nothing found to remove — this machine has neither sender nor receiver installed."
    exit 0
  fi

  blank
  echo -e "${BOLD}  Detected on this machine:${NC}"
  if [[ $have_sender -eq 1 ]]; then
    echo -e "  ${YELLOW}sender${NC}    ${SENDER_BIN}"
    echo -e "            ${NMD_SYSTEMD_UNIT}"
    echo -e "            ${NMD_SYSTEMD_UNIT_ALT} ${CYAN}(legacy)${NC}"
    echo -e "            ${NMD_CONFIG_DIR}/"
  fi
  if [[ $have_receiver -eq 1 ]]; then
    echo -e "  ${YELLOW}receiver${NC}  ${RECEIVER_BIN}"
    echo -e "            ${INSTALL_DESKTOP}/${APP_ID}.desktop"
    echo -e "            ${INSTALL_ICONS}/${APP_ID}*.svg"
  fi
  blank
  echo -e "  ${BOLD}Never removed:${NC} build packages installed for a source build, and any file"
  echo -e "  that does not belong to this app. Config and keys are confirmed individually below."
  blank

  if [[ $have_sender -eq 1 ]] && confirm "Remove the sender (nmd-service)?" Y; then
    uninstall_sender
  fi
  if [[ $have_receiver -eq 1 ]] && confirm "Remove the receiver (cosmic-applet)?" Y; then
    uninstall_receiver
  fi

  blank
  echo -e "${BOLD}${GREEN}  Uninstall complete.${NC}"
  blank
}

# ── Install ───────────────────────────────────────────────────────────────
run_install() {
  local arch
  arch=$(detect_arch)
  if [[ "$arch" == "unsupported" ]]; then
    die "Unsupported architecture: $(uname -m). Use method 2 (compile from source)."
  fi
  info "Detected architecture: ${BOLD}${arch}${NC}"

  # Prompt for component — called directly so echo output reaches the terminal.
  # Result is stored in global SELECTED_COMPONENT (not captured via $()).
  prompt_component
  local component="$SELECTED_COMPONENT"

  # Prompt for install method — same pattern.
  prompt_method "$arch"
  local method="$SELECTED_METHOD"

  blank
  info "Installing ${BOLD}${component}${NC} via ${BOLD}${method}${NC}..."
  blank

  # Warn if COSMIC isn't detected but receiver was requested
  if [[ "$component" == "receiver" || "$component" == "both" ]]; then
    if ! has cosmic-panel && ! has cosmic-comp; then
      blank
      warn "COSMIC desktop environment not detected on this machine."
      warn "The applet requires COSMIC. See: https://system76.com/cosmic"
      read -rp "  Continue anyway? [y/N]: " cont </dev/tty
      [[ "${cont:-N}" =~ ^[Yy] ]] || { info "Cancelled."; exit 0; }
    fi
  fi

  # Install binaries
  if [[ "$method" == "binary" ]]; then
    case "$component" in
      sender)   install_sender_binary   "$arch" ;;
      receiver) install_receiver_binary "$arch" ;;
      both)     install_sender_binary   "$arch"
                install_receiver_binary "$arch" ;;
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
  if [[ "$component" == "sender"   || "$component" == "both" ]]; then print_sender_done;   fi
  if [[ "$component" == "receiver" || "$component" == "both" ]]; then print_receiver_done; fi

  echo -e "${BOLD}${GREEN}  Installation complete.${NC}"
  blank
}

# ── Main ──────────────────────────────────────────────────────────────────
main() {
  parse_args "$@"
  print_header
  need_root

  case "$MODE" in
    install)   run_install ;;
    update)    run_update ;;
    uninstall) run_uninstall ;;
  esac
}

main "$@"
