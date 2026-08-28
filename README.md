# Network System Monitor

A Cosmic desktop applet that monitors all machines on your network from one panel — no SSH required.

## Status: MVP Complete 🚀

**Working Features:**
- ✅ Multi-machine metric collection (CPU, memory, disk, network, GPU, temperature)
- ✅ UDP protocol with rkyv serialization + HMAC-SHA256 authentication
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

# Generate shared HMAC secret
sudo mkdir -p /etc/nmd
sudo head -c 32 /dev/urandom | sudo xxd -p -c 32 | sudo tee /etc/nmd/secret.key
sudo chmod 600 /etc/nmd/secret.key

# Install applet to COSMIC
mkdir -p ~/.local/share/cosmic/applets
cp ./target/release/cosmic-applet ~/.local/share/cosmic/applets/network-monitor
chmod +x ~/.local/share/cosmic/applets/network-monitor

# Restart COSMIC panel to detect the applet
cosmic-panel --reload
# Then add "Network Monitor" to your panel via Panel Settings

# OR run in test mode (standalone window)
./target/release/cosmic-applet --test-mode
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
2. Prompt you to paste the HMAC secret key from desktop
3. Install and start the systemd service
4. Configure automatic startup on boot

**📖 Full deployment guide:** See [DEPLOYMENT.md](DEPLOYMENT.md)

## Overview

This project provides a single-panel view of system metrics (CPU, memory, disk, network, uptime, GPU, VRAM, temperature) across all Linux machines on your local network. Each remote machine runs a lightweight systemd service that pushes metrics to the desktop via UDP with rkyv serialization.

## Architecture

```
Remote Machine (each — Pluto, Spark, etc.)
  └── nmd-service (systemd binary)
        ↓ UDP push every 2s (rkyv + HMAC-SHA256)
Desktop Machine
  ├── cosmic-applet (panel widget + config UI)
  ├── metrics-core (shared metrics library)
  └── config: /etc/nmd/config.toml + secret.key
```

## Development Status

| Phase | Status | Description |
|-------|--------|-------------|
| 0 — Research | ✅ Complete | Reviewed minimon-applet, cosmic-lib API, established patterns |
| 1 — MVP Core | ✅ Complete | metrics-core, nmd-service, cosmic-applet working end-to-end |
| 1.5 — Production Ready | 🚧 In Progress | Installation scripts, documentation, error handling |
| 2 — Harden | ○ Next | More metrics, reliability improvements, offline detection |
| 3 — Per-machine monitors | ○ Future | Individual system monitors on each machine |
| 4 — Cosmic Utils submission | ○ Future | Submit to cosmic-utils.org for community visibility |

### What Works Today
- ✅ Multi-machine metric collection and aggregation
- ✅ Real-time panel display with threshold colors
- ✅ Configuration UI (per-sensor toggles, display options, content ordering)
- ✅ HMAC-SHA256 authentication + replay protection
- ✅ systemd service with security hardening
- ✅ Automated installation scripts
- ✅ Graduated ring chart colors (green → orange → red)
- ✅ VRAM display (GB with percentage toggle)
- ✅ Combined CPU + temperature display

### What's Left for Production

**High Priority (Week 1):**
- [ ] User documentation (troubleshooting guide, config reference)
- [ ] Fix 8 TODOs in code (error handling, offline detection)
- [ ] Test with 2-3 real remote machines
- [ ] Desktop applet installation method (not just --test-mode)

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

See [PRODUCTION-READINESS.md](PRODUCTION-READINESS.md) for complete checklist.

## Project Structure

The project follows the `pop-os/cosmic-applet-template` layout. Three crates in a single workspace:

```
network-system-monitor/
├── .planning_docs/                    # Design docs and planning materials (vault-linked)
│   ├── Brief.md, Architecture.md, Goals.md, Scope.md, Risks.md, Roadmap.md
│   ├── ImplementationGuide.md        ← Agent development reference
│   └── Index.md
├── .lavish/                          # Interactive design review artifacts (planning only)
│   └── network-monitor-design.html
├── metrics-core/                     # Shared library: system metrics collection via sysinfo + procfs
│   ├── src/{lib,cpu,memory,disk,network,uptime,gpu,temperature}.rs
│   └── benches/{cpu,memory,full_suite}_bench.rs
├── nmd-service/                      # Remote systemd service (runs on Pluto, Spark, etc.)
│   ├── src/{main,config,udp_sender,packet,metrics_aggregator,systemd_unit}.rs
│   ├── benches/{packet,aggregator}_bench.rs
│   └── install-scripts/{install.sh,generate-certs.sh,README.md}
├── cosmic-applet/                    # Desktop Cosmic applet (runs on desktop machine)
│   ├── src/{main,panel_widget,grid_window,machine_row,config_manager,udp_receiver,status_indicator,local_monitor}.rs
│   ├── benches/{panel,grid}_bench.rs
│   └── docs/{user-guide,applet-config}.md
├── Cargo.toml                        # Workspace manifest (members = [metrics-core, nmd-service, cosmic-applet])
└── .gitignore
```

> **Implementation reference**: See `.planning_docs/ImplementationGuide.md` for detailed module breakdowns, stub requirements, unit test plans, benchmarks, and documentation deliverables per agent.

## License

[To be determined]
