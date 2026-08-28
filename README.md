# Network System Monitor

A Cosmic desktop applet that monitors all machines on your network from one panel — no SSH required.

## Status: MVP Complete 🚀

**Working Features:**
- ✅ Multi-machine metric collection (CPU, memory, disk, network, GPU, temperature)
- ✅ ChaCha20-Poly1305 AEAD encryption with TOFU pairing system
- ✅ Panel widget with threshold-based ring charts (graduated green/orange/red colors)
- ✅ Configuration UI with per-sensor toggles and display options
- ✅ systemd service for remote machines
- ✅ Installation scripts (automated setup)

**Ready for Testing:** You can deploy this on your home network today!

## Quick Start

### Desktop Machine (COSMIC Desktop)

```bash
# Clone and build
git clone https://github.com/USER/network-system-monitor.git
cd network-system-monitor
cargo build --release

# Install applet to COSMIC
mkdir -p ~/.local/share/cosmic/applets
cp ./target/release/cosmic-applet ~/.local/share/cosmic/applets/network-monitor
chmod +x ~/.local/share/cosmic/applets/network-monitor

# Restart COSMIC panel to detect the applet
cosmic-panel --reload
# Then add "Network Monitor" to your panel via Panel Settings

# OR run in test mode (standalone window)
./target/release/cosmic-applet --test
```

### Remote Machines (Each Linux Machine You Want to Monitor)

```bash
# Clone repo
git clone https://github.com/USER/network-system-monitor.git
cd network-system-monitor

# Build service
cargo build --release -p nmd-service

# Run install script (will prompt for desktop IP)
sudo ./nmd-service/install-scripts/install.sh
```

The install script will:
1. Ask for your desktop machine's IP address
2. Install and start the systemd service
3. Configure automatic startup on boot

**Note:** The first connection from a remote machine triggers an automatic pairing request in the desktop applet UI. Accept it to establish secure communication.

**📖 Full deployment guide:** See [DEPLOYMENT.md](DEPLOYMENT.md)

## Overview

This project provides a single-panel view of system metrics (CPU, memory, disk, network, uptime, GPU, VRAM, temperature) across all Linux machines on your local network. Each remote machine runs a lightweight systemd service that pushes encrypted metrics to the desktop via UDP.

### Security Architecture

- **Encryption:** ChaCha20-Poly1305 AEAD (confidentiality + authenticity in one operation)
- **Pairing:** Trust-On-First-Use (TOFU) — receiver detects unknown senders, shows pairing UI
- **Replay protection:** Timestamp freshness (< 10s) + monotonic sequence numbers per session

**Pre-production note:** Currently uses `TEMP_SHARED_KEY = [0x42; 32]` as a placeholder. Per-machine ECDH-derived keys are wired up in the pairing flow but not yet enabled.

## Architecture

```
Remote Machine (each — Pluto, Spark, etc.)
  └── nmd-service (systemd binary)
        ↓ UDP push every 1s (ChaCha20-Poly1305 AEAD + TOFU pairing)
Desktop Machine
  ├── cosmic-applet (panel widget + config UI + pairing manager)
  ├── metrics-core (shared metrics library)
  └── config: ~/.config/cosmic-applet/config.toml + pairing.toml
```

## Development Status

| Phase | Status | Description |
|-------|--------|-------------|
| 0 — Research | ✅ Complete | Reviewed minimon-applet, cosmic-lib API, established patterns |
| 1 — MVP Core | ✅ Complete | metrics-core, nmd-service, cosmic-applet working end-to-end |
| 1.5 — Production Ready | 🚧 In Progress | Installation scripts, documentation, error handling |
| 2 — Per-machine ECDH keys | ○ Next | Wire up Ed25519/X25519 keypairs for true per-machine security |
| 3 — Per-machine monitors | ○ Future | Individual system monitors on each machine |
| 4 — Cosmic Utils submission | ○ Future | Submit to cosmic-utils.org for community visibility |

### What Works Today
- ✅ Multi-machine metric collection and aggregation
- ✅ Real-time panel display with threshold colors
- ✅ Configuration UI (per-sensor toggles, display options, content ordering)
- ✅ ChaCha20-Poly1305 AEAD encryption + replay protection
- ✅ systemd service with security hardening
- ✅ Automated installation scripts
- ✅ Graduated ring chart colors (green → orange → red)
- ✅ VRAM display (GB with percentage toggle)
- ✅ Combined CPU + temperature display
- ✅ TOFU pairing UI with accept/deny dropdown

### What's Left for Production

**High Priority (Week 1):**
- [ ] User documentation (troubleshooting guide, config reference)
- [ ] Fix remaining TODOs in code (error handling, offline detection)
- [ ] Test with 2-3 real remote machines
- [ ] Desktop applet installation method (not just --test mode)

**Medium Priority (Week 2):**
- [ ] Update test code for nested packet structure
- [ ] Config file validation with helpful error messages
- [ ] Launch external COSMIC system monitor from menu
- [ ] Offline machine visual indicators

**Low Priority (Future):**
- [ ] About dialog with version info
- [ ] First-run setup wizard
- [ ] Performance monitoring (log packet loss)
- [ ] Per-machine threshold configuration

## Project Structure

The project follows the `pop-os/cosmic-applet-template` layout. Three crates in a single workspace:

```
network-system-monitor/
├── metrics-core/                     # Shared library: system metrics collection via sysinfo + procfs
│   ├── src/{lib,cpu,memory,disk,network,uptime,gpu,temperature}.rs
│   └── benches/{cpu,memory,full_suite}_bench.rs
├── nmd-service/                      # Remote systemd service (runs on Pluto, Spark, etc.)
│   ├── src/{main,config,udp_sender,packet,crypto,systemd_unit}.rs
│   ├── benches/{packet,aggregator}_bench.rs
│   └── install-scripts/{install.sh,README.md}
├── cosmic-applet/                    # Desktop Cosmic applet (runs on desktop machine)
│   ├── src/{main,panel_widget,grid_window,machine_row,config_manager,
│            udp_receiver,pairing_manager,pairing_ui,status_indicator}.rs
│   ├── benches/{panel,grid}_bench.rs
│   └── docs/{user-guide,applet-config}.md
├── docs/
│   └── PAIRING-SYSTEM-V1.md        # Encryption + pairing system specification
├── Cargo.toml                        # Workspace manifest (members = [metrics-core, nmd-service, cosmic-applet])
└── .gitignore
```

## License

MIT OR Apache-2.0
