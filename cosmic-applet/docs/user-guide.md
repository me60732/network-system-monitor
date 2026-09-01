# Network System Monitor — User Guide

> **Status**: Stub documentation (Troi to complete)
> Last updated: 2026-08-20

This guide explains how to install, configure, and use the Network System Monitor Cosmic applet.

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Panel Widget Overview](#panel-widget-overview)
4. [Grid Window](#grid-window)
5. [Adding Remote Machines](#adding-remote-machines)
6. [Metric Selection](#metric-selection)
7. [Color Thresholds & Status Indicators](#color-thresholds--status-indicators)
8. [Troubleshooting](#troubleshooting)

---

## Installation

### Prerequisites

- A Linux desktop running the Cosmic desktop environment (Pop!_OS 24.04+ or COSMIC DE).
- Remote machines must have `nmd-service` installed and running as a systemd service.

### Install from Source

```bash
git clone https://github.com/mark/network-system-monitor.git
cd network-system-monitor/cosmic-applet
just build-release && just install
```

This installs the applet binary to `~/.local/bin/` and registers it with the Cosmic panel.

---

## Quick Start

After installation, add the "Network System Monitor" widget to your Cosmic panel:

1. Right-click on an empty area of the Cosmic panel.
2. Select **Add Applet** → find "Network System Monitor".
3. The applet appears in the panel showing desktop stats (CPU, memory, disk, etc.).

Clicking the panel widget opens a grid window listing all configured remote machines.

---

## Panel Widget Overview

The panel widget displays a single-line summary of local system metrics:

```
[CPU: 23% | MEM: 45% | DISK: 67% | NET: ↗ 1.2MB/s | UP: 2h | GPU: 512MB | TEMP: 65°C]
```

### Color Thresholds

Each percentage-based metric is color-coded based on severity:

| Range       | Color   | Meaning                          |
|-------------|---------|----------------------------------|
| < 60%       | Green   | Normal — healthy operating range |
| 60–80%      | Yellow  | Warning — approaching capacity   |
| > 80%       | Red     | Critical — immediate attention   |

---

## Panel Widget & Machine List

The panel widget displays a single-line summary of local system metrics. Clicking it opens a **machine list** (not a grid window) that always shows all registered remote machines.

### UI Flow

1. **Machine List**: Always visible — one row per machine with status indicators
2. **Click Machine Row**: Opens **machine detail** view
3. **Machine Detail Top**: Shows the sensor panel row (configured via sensor config)
4. **Machine Detail Below**: All metrics NOT visible in the panel row
5. **Gear Icon (⚙)**: Opens per-machine sensor configuration menu

### Status Indicators (per machine row)

- **●** — Machine is online (recent UDP packet received within timeout).
- **○** — Machine is offline or pending first-packet registration.

---

## Adding Remote Machines

To monitor a remote machine:

1. Ensure `nmd` is installed and running on the target machine.
2. In the machine list UI, click **+ Add Machine** or edit config.toml directly.
3. Enter the machine's hostname/IP address and confirm it matches the `machine_id` in nmd-service's config.
4. The machine appears as **Pending** (○) until its first UDP packet arrives.

---

## Per-Machine Sensor Configuration

What shows in the panel row is configured per-machine via the gear icon (⚙) in machine detail view:

- CPU usage percentage (ring chart + label)
- Memory used percentage (ring chart + label, configurable as % or GB)
- Disk usage and I/O (text-only, not in panel row by default)
- Network RX/TX rates (ring chart + adaptive units)
- Uptime (text-only)
- GPU VRAM usage (ring chart + label, configurable as % or GB)
- Temperature (ring chart + custom °C text)

**Global Settings**: Value size, monospace font, panel spacing, content order — apply to ALL machines via Settings UI.

---

## Color Thresholds & Status Indicators

All metrics use consistent color coding across the panel widget and grid window:

| Threshold | Meaning         | Panel Widget  | Grid Window    |
|-----------|-----------------|---------------|----------------|
| < 60%     | Normal          | Green text    | Green progress bar |
| 60–80%    | Warning         | Yellow text   | Yellow progress bar |
| > 80%     | Critical        | Red text      | Red progress bar    |

---

## Troubleshooting

### Machine shows ○ (offline) in the machine list

- Verify `nmd` is running on the remote machine: `systemctl status nmd`.
- Check that UDP port 51057 is open between machines.
- Ensure the receiver's X25519 pubkey is correctly set in sender config (`receiver_pubkey = "<hex>"` in `/etc/nmd/config.toml`).

### No metrics appear in the panel widget

- Confirm nmd-service is sending packets: check sender logs.
- Check applet logs: look for "Packet received" messages.
- Verify pairing was accepted: check `~/.config/cosmic-applet/pairing.toml`.

---

*This is a stub — Troi will complete with full screenshots, keyboard shortcuts, and advanced configuration options.*