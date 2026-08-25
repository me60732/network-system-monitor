# Network System Monitor — Applet Configuration Reference

> **Status**: Stub documentation (Troi to complete)
> Last updated: 2026-08-20

This document describes the TOML configuration file format for the cosmic-applet, which extends minimon-applet's config structure with per-machine metric selection and UDP receiver settings.

## Table of Contents

1. [Configuration File Location](#configuration-file-location)
2. [Top-Level Options](#top-level-options)
3. [Machine Entries](#machine-entries)
4. [Per-Machine Metric Selection](#per-machine-metric-selection)
5. [Default Configuration Example](#default-configuration-example)
6. [Secret Key Management](#secret-key-management)

---

## Configuration File Location

The applet looks for `config.toml` in the following locations, in order:

1. Path specified via command-line argument (`--config /path/to/config.toml`).
2. `~/.config/network-system-monitor/config.toml`.
3. `./config.toml` (current working directory).

If no config file is found, a default configuration with only `localhost` as a monitored machine is used.

---

## Top-Level Options

| Option             | Type    | Default                          | Description                                  |
|--------------------|---------|----------------------------------|----------------------------------------------|
| `udp_port`         | integer | `51057`                          | UDP port the applet listens on for incoming MetricPacket traffic. Must match nmd-service's send port. |
| `hmac_secret_path` | string  | `/etc/nmd/secret.key`            | Path to the HMAC-SHA256 pre-shared key file (32 bytes). Shared with all remote machines. |
| `auto_expand_grid` | boolean | `true`                           | Whether the grid window auto-expands when a new machine comes online. Set to `false` for manual toggle only. |

---

## Machine Entries

The `[machines]` section contains an array of machine configurations. Each entry extends minimon-applet's format with additional fields.

| Option        | Type    | Default  | Description                                  |
|---------------|---------|----------|----------------------------------------------|
| `name`        | string  | (required) | Unique identifier — must match nmd-service's `machine_id`. Used for UDP packet routing and HMAC replay protection. |
| `enabled`     | boolean | `true`   | Whether this machine is actively monitored. Disable to temporarily exclude without deleting config. |
| `host`        | string  | (required) | IP address or hostname of the remote machine running nmd-service. |
| `port`        | integer | `51057`  | UDP port on the remote machine's nmd-service sender. Usually matches `udp_port`. |

### Example Machine Entry

```toml
[[machines]]
name = "pluto"
enabled = true
host = "192.168.1.20"
port = 51057
show_cpu = true
show_memory = true
# ... (see per-machine metric selection below)
```

---

## Per-Machine Metric Selection

Each machine can have individual metrics shown or hidden in the grid window via boolean checkbox fields. All default to `true` if omitted.

| Option         | Type    | Default  | Description                                    |
|----------------|---------|----------|-------------------------------------------------|
| `show_cpu`     | boolean | `true`   | Display CPU usage percentage column for this machine. |
| `show_memory`  | boolean | `true`   | Display memory used percentage column.          |
| `show_disk`    | boolean | `true`   | Display disk usage percentage column.           |
| `show_network` | boolean | `true`   | Display network RX/TX bytes column.              |
| `show_uptime`  | boolean | `true`   | Display uptime column (human-readable).          |
| `show_gpu_vram`| boolean | `true`   | Display GPU VRAM usage in MB column.            |
| `show_temperature` | boolean | `true` | Display temperature (°C) column.                |

### Example: Hide GPU Column for a Machine Without Discrete Graphics

```toml
[[machines]]
name = "spark"
host = "192.168.1.30"
show_gpu_vram = false    # This machine has no discrete GPU — hide the column
show_temperature = false  # No thermal sensors available on this hardware
```

---

## Default Configuration Example

When no config file is found, the applet generates a default `config.toml` with only localhost:

```toml
# Network System Monitor — cosmic-applet configuration
udp_port = 51057
hmac_secret_path = "/etc/nmd/secret.key"
auto_expand_grid = true

[[machines]]
name = "localhost"
enabled = true
host = "127.0.0.1"
port = 51057
show_cpu = true
show_memory = true
show_disk = true
show_network = true
show_uptime = true
show_gpu_vram = true
show_temperature = true
```

---

## Secret Key Management (Worf Phase 1A)

The HMAC pre-shared key is stored at the path specified by `hmac_secret_path` (default: `/etc/nmd/secret.key`). It must be exactly **32 bytes** for HMAC-SHA256. This file is generated on each remote machine by the install script (`install-scripts/install.sh`) and shared out-of-band to the desktop applet during setup.

### Generating a Key Manually

```bash
# Generate a 32-byte random key and set restrictive permissions:
dd if=/dev/urandom of=/etc/nmd/secret.key bs=1 count=32
chmod 0600 /etc/nmd/secret.key
chown root:root /etc/nmd/secret.key
```

---

*This is a stub — Troi will add advanced options, environment variable overrides, and config migration notes.*