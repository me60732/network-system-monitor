# Configuration Reference

Complete reference for Network System Monitor configuration files.

## File Locations

### Desktop Machine (cosmic-applet)
- **Config file**: `/etc/nmd/config.toml`
- **HMAC secret**: `/etc/nmd/secret.key` (0600 permissions)
- **User config**: `~/.config/cosmic-applet/minimon.toml` (UI preferences)

### Remote Machines (nmd-service)
- **Config file**: `/etc/nmd/config.toml`
- **HMAC secret**: `/etc/nmd/secret.key` (0600 permissions, must match desktop)

---

## Desktop Configuration (`/etc/nmd/config.toml`)

### UDP Receiver Section

```toml
[udp_receiver]
port = 51057                                 # UDP port to listen on
hmac_secret_path = "/etc/nmd/secret.key"     # Path to HMAC secret key file
```

**port** (integer, default: 51057)
- UDP port for receiving metrics from remote machines
- Must be accessible from all remote machines on your network
- Firewall rule required: `sudo ufw allow 51057/udp`

**hmac_secret_path** (string, required)
- Absolute path to the HMAC secret key file
- File must contain 32 bytes of random hex data (64 hex characters)
- Permissions must be 0600 (read/write for owner only)
- Must be identical on all machines (desktop + all remotes)

### Machine Registration

```toml
[[machines]]
name = "desktop"
host = "127.0.0.1"
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
- Must match the `machine_name` in remote machine's config

**host** (string, required)
- IP address or hostname
- Not currently used for connection (UDP is push-based)
- Reserved for future features (offline detection, SSH integration)

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
[service]
machine_name = "pluto"                       # Unique machine identifier
destination = "192.168.1.100:51057"          # Desktop IP:port
interval_ms = 2000                           # Metrics push interval
hmac_secret_path = "/etc/nmd/secret.key"     # Path to HMAC secret
```

**machine_name** (string, required)
- Unique identifier for this machine
- Must match a machine name in desktop config (or will auto-register)
- Used for identification in UDP packets

**destination** (string, required)
- Desktop machine IP address and UDP port
- Format: `IP:PORT` (e.g., `192.168.1.100:51057`)
- Must be reachable from this machine

**interval_ms** (integer, default: 2000)
- How often to collect and send metrics (milliseconds)
- Recommended: 2000-5000 (2-5 seconds)
- Lower values = more network traffic, more CPU usage
- Higher values = less responsive UI updates

**hmac_secret_path** (string, required)
- Absolute path to HMAC secret key file
- File must contain the same 32-byte hex key as desktop machine
- Permissions must be 0600

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

## HMAC Secret Key

The HMAC secret key file authenticates all metrics packets between remote machines and the desktop.

### Generate Secret Key

```bash
sudo mkdir -p /etc/nmd
sudo head -c 32 /dev/urandom | sudo xxd -p -c 32 | sudo tee /etc/nmd/secret.key
sudo chmod 600 /etc/nmd/secret.key
```

### Format
- 32 bytes (256 bits) of random data
- Stored as 64 hexadecimal characters
- Example: `a3f2c91d8e7b6f5a4d3c2b1a0987654fedcba9876543210fedcba987654321`

### Security
- Treat this like a password — anyone with the key can send fake metrics
- Use 0600 permissions (owner read/write only)
- Generated from `/dev/urandom` (cryptographically secure)
- Copy to all machines manually (do not transmit over insecure channels)

### Copying to Remote Machines

**Via SSH (recommended):**
```bash
# On desktop
cat /etc/nmd/secret.key

# On remote machine
sudo tee /etc/nmd/secret.key <<EOF
<paste key here>
EOF
sudo chmod 600 /etc/nmd/secret.key
```

**Via USB drive (air-gapped networks):**
```bash
# Copy key to USB
cp /etc/nmd/secret.key /media/usb/

# On remote machine
sudo cp /media/usb/secret.key /etc/nmd/
sudo chmod 600 /etc/nmd/secret.key
```

---

## User Preferences (`~/.config/cosmic-applet/minimon.toml`)

Generated automatically by the applet UI. Contains per-sensor display preferences.

### Example Structure

```toml
[cpu]
chart_visible = true
label_visible = true
icon_visible = true
percentage = false

[cpu_temp]
chart_visible = true
label_visible = true
icon_visible = true

[memory]
chart_visible = true
label_visible = true
icon_visible = true
percentage = true

[gpu_load]
chart_visible = true
label_visible = true
icon_visible = true

[gpu_vram]
chart_visible = true
label_visible = true
icon_visible = true
percentage = false  # Show GB instead of percentage

[network]
chart_visible = true
label_visible = true
icon_visible = true

[disk]
chart_visible = true
label_visible = true
icon_visible = true

[content_order]
order = ["cpu", "cpu_temp", "memory", "gpu_load", "gpu_vram", "network", "disk"]
```

### Sensor Options

**chart_visible** (boolean, default: true)
- Whether to display the ring chart for this sensor

**label_visible** (boolean, default: true)
- Whether to display the text label

**icon_visible** (boolean, default: true)
- Whether to display the icon

**percentage** (boolean, default varies)
- For memory: show as percentage of total RAM
- For GPU VRAM: show as percentage of total VRAM
- Default: `false` for VRAM (shows GB), `true` for memory

### Content Order

**order** (array of strings)
- Order sensors are displayed in the panel (left to right)
- Sensors not in list are hidden
- Edit via applet config UI (Settings → Content Order)

---

## Troubleshooting

### Config File Validation

```bash
# Check desktop config
cat /etc/nmd/config.toml

# Check remote config
ssh remote-machine cat /etc/nmd/config.toml

# Verify HMAC keys match
diff <(cat /etc/nmd/secret.key) <(ssh remote-machine cat /etc/nmd/secret.key)
```

### Common Config Errors

**"HMAC verification failed"**
- Cause: Secret keys don't match between desktop and remote
- Fix: Copy the same secret.key to all machines

**"Permission denied reading secret key"**
- Cause: File permissions too restrictive or key file missing
- Fix: `sudo chmod 600 /etc/nmd/secret.key`

**"No metrics received"**
- Cause: Firewall blocking UDP port, wrong destination IP, or service not running
- Fix: Check firewall (`sudo ufw allow 51057/udp`), verify destination IP, check service status (`systemctl status nmd-service`)

**"Invalid TOML syntax"**
- Cause: Typo in config file
- Fix: Use a TOML validator or check syntax carefully (quotes, brackets, commas)

---

## Advanced Configuration

### Custom UDP Port

If port 51057 conflicts with another service:

1. Desktop `/etc/nmd/config.toml`:
   ```toml
   [udp_receiver]
   port = 52000  # New port
   ```

2. Remote `/etc/nmd/config.toml`:
   ```toml
   [service]
   destination = "192.168.1.100:52000"  # Match desktop port
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
gpu = false
temperature = false
```

### Adjust Push Interval

For high-latency or low-bandwidth networks:

```toml
[service]
interval_ms = 5000  # 5 seconds instead of 2
```

---

## See Also

- [DEPLOYMENT.md](DEPLOYMENT.md) — Installation and setup guide
- [README.md](README.md) — Project overview
- [PRODUCTION-READINESS.md](PRODUCTION-READINESS.md) — Status and roadmap
