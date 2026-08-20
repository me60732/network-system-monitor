# Network System Monitor

A Cosmic desktop applet that monitors all machines on your network from one panel — no SSH required.

## Overview

This project provides a single-panel view of system metrics (CPU, memory, disk, network, uptime, GPU, VRAM, temperature) across all Linux machines on your local network. Each remote machine runs a lightweight systemd service that pushes metrics to the desktop via UDP with rkyv serialization.

## Architecture

```
Remote Machine (each — Pluto, Spark, etc.)
  └── nmd-service (systemd binary)
        ↓ UDP push (rkyv-encoded + machine ID)
Desktop Machine
  ├── cosmic-applet (panel widget → click expands grid window)
  ├── metrics-core (shared library: sysinfo + procfs collection)
  ├── nmd-config (TOML-based config, extends minimon-applet format)
  └── nmd-protocol (rkyv serialization definitions)
```

## Quick Start

> **Note**: This is currently in the planning/design phase. No code has been written yet.

1. Clone this repo
2. Review `.planning_docs/` for architecture and design decisions
3. View interactive design review at `.lavish/network-monitor-design.html`

## Development Phases

| Phase | Status | Description |
|-------|--------|-------------|
| 0 — Research | ✅ Complete | Reviewed minimon-applet, cosmic-lib API, established patterns |
| 1 — MVP | 📋 Ready | Implementation Guide complete. metrics-core → nmd-service → cosmic-applet with TNG agent workflow handoffs (Geordi→Beverly→Worf→Troi) |
| 2 — Harden | ○ Pending | More metrics, reliability improvements |
| 3 — Per-machine monitors | ○ Future | Individual system monitors on each machine |
| 4 — Cosmic Utils submission | ○ Future | Submit to cosmic-utils.org for community visibility |

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
