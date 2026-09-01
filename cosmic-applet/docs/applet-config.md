# Network System Monitor — Applet Configuration Reference

This document describes the TOML configuration file format for the cosmic-applet.

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
| `auto_expand_grid` | boolean | `true`                           | Whether the machine list window auto-expands when a new machine comes online. Set to `false` for manual toggle only. |

---

## Machine Entries

The `[machines]` section contains an array of machine configurations.

| Option        | Type    | Default  | Description                                  |
|---------------|---------|----------|----------------------------------------------|
| `name`        | string  | (required) | Unique identifier — must match nmd-service's `machine_id`. Used for UDP packet routing and replay protection. |
| `enabled`     | boolean | `true`   | Whether this machine is actively monitored. Disable to temporarily exclude without deleting config. |
| `host`        | string  | (required) | IP address or hostname of the remote machine running nmd-service. |
| `port`        | integer | `51057`  | UDP port on the remote machine's nmd-service sender. Usually matches `udp_port`. |
| `sensor_config` | object | (optional) | Per-machine sensor configuration (what shows in panel row via gear icon). |

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

## Per-Machine Sensor Configuration

Each machine has its own `MachineSensorConfig` stored in MachineConfig. What shows in the panel row is configured per-machine via the gear icon in machine detail view.

### Example Per-Machine Config with Sensor Configuration

```toml
[[machines]]
name = "spark"
host = "192.168.1.30"
enabled = true

# Per-machine sensor configuration (what shows in panel row)
sensor_config = {
  cpu_chart_visible = true,
  cpu_label_visible = true,
  memory_chart_visible = true,
  memory_percentage = true,  # Show as percentage
  disk_chart_visible = false,  # Disk not shown in row
  network_chart_visible = true,
  gpu_load_chart_visible = false,  # No GPU on this machine
  gpu_vram_chart_visible = false,
  temperature_chart_visible = true,
}
```

---

## Default Configuration Example

When no config file is found, the applet generates a default `config.toml` with only localhost:

```toml
# Network System Monitor — cosmic-applet configuration
udp_port = 51057
auto_expand_grid = true

[[machines]]
name = "localhost"
enabled = true
host = "127.0.0.1"
port = 51057
sensor_config = {
  cpu_chart_visible = true,
  memory_chart_visible = true,
  disk_chart_visible = false,
  network_chart_visible = true,
  gpu_load_chart_visible = true,
  gpu_vram_chart_visible = true,
  temperature_chart_visible = true,
}
```

---

## Pairing System

The ChaCha20-Poly1305 AEAD encryption uses ECDH-derived per-machine shared keys stored in `~/.config/cosmic-applet/pairing.toml`. The sender's `receiver_pubkey` field in `/etc/nmd/config.toml` is set automatically via TCP pairing on first start.

### Generating a Key Manually

```bash
# Generate a 32-byte random key and set restrictive permissions:
dd if=/dev/urandom of=/etc/nmd/secret.key bs=1 count=32
chmod 0600 /etc/nmd/secret.key
chown root:root /etc/nmd/secret.key
```

---

*This is a stub — Troi will add advanced options, environment variable overrides, and config migration notes.*