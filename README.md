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
| 1 — MVP | 🚧 Planning | Applet + network visibility with Pluto/Spark |
| 2 — Harden | ○ Pending | More metrics, reliability improvements |
| 3 — Per-machine monitors | ○ Future | Individual system monitors on each machine |
| 4 — Cosmic Utils submission | ○ Future | Submit to cosmic-utils.org for community visibility |

## Project Structure (Planned)

```
network-system-monitor/
├── .planning_docs/          # Design docs and planning materials
│   ├── Brief.md
│   ├── Architecture.md
│   ├── Goals.md
│   ├── Scope.md
│   ├── Risks.md
│   ├── Roadmap.md
│   └── Index.md
├── .lavish/                 # Interactive design review artifacts
│   └── network-monitor-design.html
├── metrics-core/            # Shared library for system metrics collection (TBD)
├── nmd-service/             # Systemd service binary for remote machines (TBD)
├── cosmic-applet/           # Desktop Cosmic applet (TBD)
└── install-scripts/         # Remote machine setup scripts (TBD)
```

## License

[To be determined]
