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

## Grid Window

Clicking the panel widget expands into a grid window showing all registered remote machines.

### Columns

| Column    | Description                                    |
|-----------|-------------------------------------------------|
| Name      | Machine hostname with online/offline indicator  |
| CPU       | Aggregate usage percentage (0–100%)             |
| Memory    | RAM used as percentage of total                  |
| Disk      | Root partition usage percentage                  |
| Network   | RX/TX byte counters since boot                   |
| Uptime    | Time since last reboot (human-readable)          |
| GPU VRAM  | Video memory used in MB (if discrete GPU present)|
| Temperature | CPU/GPU temp in Celsius                        |

### Status Indicators

- **●** — Machine is online (recent UDP packet received within timeout).
- **○** — Machine is offline or pending first-packet registration.

---

## Adding Remote Machines

To monitor a remote machine:

1. Ensure `nmd-service` is installed and running on the target machine.
2. In the grid window, click **+ Add Machine**.
3. Enter the machine's hostname/IP address and confirm it matches the `machine_id` in nmd-service's config.
4. The machine appears as **Pending** (○) until its first UDP packet arrives.

---

## Metric Selection

Each machine can have individual metrics shown or hidden via checkbox selection:

- CPU usage percentage
- Memory used percentage
- Disk usage percentage
- Network RX/TX bytes
- Uptime
- GPU VRAM usage
- Temperature

Uncheck a metric to hide its column for that specific machine. Changes are saved automatically to `config.toml`.

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

### Machine shows ○ (offline) in the grid window

- Verify `nmd-service` is running on the remote machine: `systemctl status nmd.service`.
- Check that UDP port 51057 is open between machines.
- Ensure the HMAC secret key at `/etc/nmd/secret.key` matches on both desktop and remote machine.

### No metrics appear in the panel widget

- Confirm `metrics-core` can read system files (`/proc/stat`, `/proc/meminfo`).
- Check applet logs: `journalctl -f /cosmic-applet`.

---

*This is a stub — Troi will complete with full screenshots, keyboard shortcuts, and advanced configuration options.*