# Configuration Reference

Complete reference for Network System Monitor configuration files.

## File Locations

### Desktop Machine (cosmic-applet)
- **Config file**: `~/.config/cosmic-applet/config.toml` or `config.toml` in working directory
- **Pairing storage**: `~/.config/cosmic-applet/pairing.toml`
- **Receiver keypair**: Auto-generated at `~/.config/cosmic-applet/receiver.key` (Ed25519)

### Remote Machines (nmd service)
- **Config file**: `/etc/nmd/config.toml`
- **Sender keypair**: Auto-generated at `~/.config/nmd/keypair.key` (Ed25519)

---

## Desktop Configuration (`~/.config/cosmic-applet/config.toml` or `config.toml`)

### UDP Receiver Section

```toml
udp_port = 51057                                 # UDP port to listen on
```

**udp_port** (integer, default: 51057)
- UDP port for receiving metrics from remote machines
- Must be accessible from all remote machines on your network
- Firewall rule required: `sudo ufw allow 51057/udp`

### Machine Registration

```toml
[[machines]]
name = "desktop"
host = "127.0.0.1"
port = 51057
enabled = true

[machines.metrics]
cpu = true
memory = true
disk = true
network = true
uptime = true
gpu_vram = true
temperature = true

[[machines]]
name = "pluto"
host = "192.168.1.50"
port = 51057
enabled = true

[machines.metrics]
cpu = true
memory = true
disk = true
network = true
uptime = true
gpu_vram = false  # Pluto has no GPU
temperature = true
```

**name** (string, required)
- Unique identifier for this machine
- Used for display in the applet
- Must match the `machine_id` in remote machine's config

**host** (string, required)
- IP address or hostname
- Not currently used for connection (UDP is push-based)
- Reserved for future features (offline detection, SSH integration)

**port** (integer, default: 51057)
- UDP port on the remote nmd instance

**enabled** (boolean, default: true)
- Whether to display this machine in the applet
- Setting to `false` hides the machine without removing config

**metrics.*** (boolean, default: true)
- Individual toggles for each metric type
- `cpu`: CPU usage percentage
- `memory`: RAM usage
- `disk`: Disk usage and I/O
- `network`: Network RX/TX rates
- `uptime`: System uptime
- `gpu_vram`: GPU VRAM usage (set to `false` for machines without GPU)
- `temperature`: CPU temperature

---

## Remote Machine Configuration (`/etc/nmd/config.toml`)

### Service Section

```toml
host = "192.168.1.100"           # Desktop IP address
port = 51057                     # Desktop UDP port
refresh_interval_secs = 1        # Metrics push interval (seconds)
machine_id = "pluto"             # Unique machine identifier
```

**host** (string, required)
- Desktop machine IP address to send UDP packets to
- Format: IP string (e.g., `192.168.1.100`)
- Must be reachable from this machine

**port** (integer, default: 51057)
- UDP port on the desktop applet

**refresh_interval_secs** (integer, default: 1)
- How often to collect and send metrics (seconds)
- Recommended: 1-5 seconds
- Lower values = more network traffic, more CPU usage
- Higher values = less responsive UI updates

**machine_id** (string, required)
- Unique identifier for this machine
- Auto-detected from hostname if not set
- Used for identification in UDP packets and pairing flow

### Metrics Section

```toml
[metrics]
cpu = true
memory = true
disk = true
network = true
uptime = true
gpu = true
temperature = true
```

**cpu** (boolean, default: true)
- Collect CPU usage statistics
- Uses /proc/stat for accurate delta measurements

**memory** (boolean, default: true)
- Collect memory usage (RAM + swap)
- Uses sysinfo crate

**disk** (boolean, default: true)
- Collect disk usage and I/O statistics
- Includes partition information for all mounted filesystems

**network** (boolean, default: true)
- Collect network RX/TX byte rates
- Aggregates across all active interfaces

**uptime** (boolean, default: true)
- System uptime in seconds
- Read from /proc/uptime

**gpu** (boolean, default: true)
- GPU usage and VRAM statistics
- Requires NVIDIA GPU + nvml-wrapper
- Automatically disabled if no GPU detected

**temperature** (boolean, default: true)
- CPU temperature in Celsius
- Uses hwmon sysfs interface on Linux
- May require specific hardware support

---

## Pairing System Configuration (`~/.config/cosmic-applet/pairing.toml`)

### Format

```toml
# Machine pairing registry
# Generated automatically when accept pairing UI prompt

[[paired_machines]]
machine_id = "pluto"
shared_key = "a1b2c3d4e5f6..."  # 32-byte ChaCha20 key (64 hex chars)
paired_at = "2026-08-28T14:32:00Z"
host = "192.168.1.100"

[[paired_machines]]
machine_id = "server-alpha"
shared_key = "1a2b3c4d5e6f..."
paired_at = "2026-08-27T09:15:00Z"
host = "192.168.1.50"
```

**machine_id** (string, required)
- Unique identifier of the paired machine

**shared_key** (hex string, 64 chars)
- 32-byte ChaCha20 key derived via ECDH during pairing
- Hex-encoded for TOML storage

**paired_at** (ISO 8601 datetime string)
- Timestamp when pairing was established

**host** (string)
- IP address or hostname of the paired machine at time of pairing

### Permissions: `0600` (critical — contains shared encryption keys)

---

## Key Generation

### Sender Keypair (`~/.config/nmd/keypair.key`)
- Ed25519 identity keypair (64 bytes: 32 private + 32 public)
- Auto-generated on first nmd start
- Used for ECDH key derivation during pairing
- Permissions: `0600` (owner read/write only)

### Receiver Keypair (`~/.config/cosmic-applet/receiver.key`)
- Ed25519 identity keypair (auto-generated)
- Used to verify sender ECDH key during pairing
- Permissions: `0600`

---

## Pairing System Configuration (`~/.config/cosmic-applet/pairing.toml`)

### Format

```toml
# Machine pairing registry
# Generated automatically when accept pairing UI prompt in the applet

[[paired_machines]]
machine_id = "pluto"
shared_key = "a1b2c3d4e5f6..."  # 32-byte ChaCha20 key (64 hex chars)
paired_at = "2026-08-28T14:32:00Z"
host = "192.168.1.100"

[[paired_machines]]
machine_id = "server-alpha"
shared_key = "1a2b3c4d5e6f..."
paired_at = "2026-08-27T09:15:00Z"
host = "192.168.1.50"
```

**machine_id** (string, required)
- Unique identifier of the paired machine (must match nmd's `machine_id`)

**shared_key** (hex string, 64 chars)
- 32-byte ChaCha20 key derived via ECDH during pairing
- Hex-encoded for TOML storage

**paired_at** (ISO 8601 datetime string)
- Timestamp when pairing was established

**host** (string)
- IP address or hostname of the paired machine at time of pairing

---

## Per-Machine Sensor Configuration

Each machine has its own `MachineSensorConfig` stored in `MachineConfig`. What shows in the machine row is configured per-machine via the gear icon in `machine_detail`.

### Example Machine Config Structure

```toml
[[machines]]
name = "pluto"
host = "192.168.1.50"
enabled = true

# Per-machine sensor configuration (what shows in panel row)
sensor_config = {
  cpu_chart_visible = true,
  cpu_label_visible = true,
  memory_chart_visible = true,
  memory_percentage = true,
  disk_chart_visible = false,  # Disk not shown in row
  network_chart_visible = true,
  gpu_load_chart_visible = true,
  gpu_vram_chart_visible = true,
  temperature_chart_visible = true,
}

# Refresh rate per machine (configured via nmd config file)
refresh_interval_secs = 1
```

### Per-Machine Sensor Options

**chart_visible** (boolean, default: true)
- Whether to display the ring chart for this sensor in the panel row

**label_visible** (boolean, default: true)
- Whether to display the text label for this sensor

**percentage** (boolean, default varies)
- For memory: show as percentage of total RAM
- For GPU VRAM: show as percentage of total VRAM
- Default: `false` for VRAM (shows GB), `true` for memory

---

## Troubleshooting

### Config File Validation

```bash
# Check desktop config
cat ~/.config/cosmic-applet/config.toml

# Check remote config
ssh remote-machine cat /etc/nmd/config.toml

# Verify machine_id matches between configs
grep machine_id /etc/nmd/config.toml
```

### Common Config Errors

**"No metrics received"**
- Cause: Firewall blocking UDP port, wrong destination IP, or service not running
- Fix: Check firewall (`sudo ufw allow 51057/udp`), verify destination IP, check service status (`systemctl status nmd`)

**"Permission denied reading config"**
- Cause: File permissions too restrictive or file missing
- Fix: Ensure config files are readable by the running user

**"Invalid TOML syntax"**
- Cause: Typo in config file
- Fix: Use a TOML validator or check syntax carefully (quotes, brackets, commas)

**"Pairing request not appearing"**
- Cause: Receiver not listening or packet decryption failed
- Fix: Check applet logs (`./cosmic-applet --test`), verify UDP port is open

---

## Advanced Configuration

### Custom UDP Port

If port 51057 conflicts with another service:

1. Desktop `~/.config/cosmic-applet/config.toml`:
   ```toml
   udp_port = 52000  # New port
   ```

2. Remote `/etc/nmd/config.toml`:
   ```toml
   host = "192.168.1.100"
   port = 52000  # Match desktop port
   ```

3. Update firewall:
   ```bash
   sudo ufw allow 52000/udp
   ```

### Disable Specific Metrics Per Machine

To reduce CPU usage or network traffic:

```toml
# On remote machine with no GPU
[metrics]
gpu = false
gpu_vram = false

# Minimal metrics (CPU and memory only)
[metrics]
cpu = true
memory = true
disk = false
network = false
uptime = false
temperature = false
```

### Adjust Push Interval

For high-latency or low-bandwidth networks:

```toml
refresh_interval_secs = 5  # 5 seconds instead of 1
```

---

## See Also

- [DEPLOYMENT.md](DEPLOYMENT.md) — Installation and setup guide
- [README.md](README.md) — Project overview
- [docs/PAIRING-SYSTEM-V1.md](docs/PAIRING-SYSTEM-V1.md) — Encryption + pairing system specification
