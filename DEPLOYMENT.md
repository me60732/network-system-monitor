# Deployment Guide

Complete guide for deploying Network System Monitor across your home network.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│ Desktop Machine (COSMIC Desktop)                                │
│  - cosmic-applet (panel widget + config UI)                     │
│  - UDP receiver on port 51057                                   │
│  - HMAC secret key: /etc/nmd/secret.key                         │
└─────────────────────────────────────────────────────────────────┘
                              ▲
                              │ UDP packets every 2s
                              │ (rkyv + HMAC-SHA256)
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
┌───────▼────────┐    ┌───────▼────────┐    ┌───────▼────────┐
│ Remote Machine │    │ Remote Machine │    │ Remote Machine │
│ (Pluto)        │    │ (Spark)        │    │ ...            │
│ nmd-service    │    │ nmd-service    │    │ nmd-service    │
│ systemd daemon │    │ systemd daemon │    │ systemd daemon │
└────────────────┘    └────────────────┘    └────────────────┘
```

## Prerequisites

### Desktop Machine
- COSMIC Desktop environment
- Rust toolchain (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- UDP port 51057 accessible from local network

### Remote Machines (each)
- Linux (any distribution)
- Rust toolchain (for building from source)
- systemd
- Network access to desktop machine

## Installation Steps

### 1. Desktop Machine Setup

#### Build the applet

```bash
git clone https://github.com/USER/network-system-monitor.git
cd network-system-monitor
cargo build --release -p cosmic-applet
```

#### Generate shared HMAC secret

```bash
sudo mkdir -p /etc/nmd
# SEC-01: Generate 32 raw bytes (not hex-encoded)
(umask 077 && sudo head -c 32 /dev/urandom | sudo tee /etc/nmd/secret.key > /dev/null)
# Fix ownership for desktop user access
sudo chown $USER:$USER /etc/nmd/secret.key
sudo chmod 600 /etc/nmd/secret.key
```

**Save this key — you'll need to copy it to every remote machine using `scp` or similar.**

#### Create config file

```bash
sudo tee /etc/nmd/config.toml <<EOF
[udp_receiver]
port = 51057
hmac_secret_path = "/etc/nmd/secret.key"

# Remote machines will auto-register on first packet
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
EOF
```

#### Install and run applet

**Option A: Test Mode (Development)**

For quick testing without full installation:

```bash
./target/release/cosmic-applet --test-mode
```

This runs the applet in a standalone window without COSMIC panel integration.

**Option B: Manual Installation (Production)**

COSMIC applets can be installed to your local user directory:

```bash
# Create applet directory if it doesn't exist
mkdir -p ~/.local/share/cosmic/applets

# Copy the compiled binary
cp ./target/release/cosmic-applet ~/.local/share/cosmic/applets/network-monitor

# Make it executable
chmod +x ~/.local/share/cosmic/applets/network-monitor
```

**Registering with COSMIC Panel:**

After copying the binary, restart the COSMIC panel to discover the new applet:

```bash
# Restart COSMIC panel (or log out and back in)
cosmic-panel --reload
# OR
systemctl --user restart cosmic-panel.service
```

Then add it to your panel:
1. Right-click the COSMIC panel
2. Select "Panel Settings" → "Add Applet"
3. Find "Network Monitor" in the list
4. Click to add it to your panel

**Option C: System-wide Installation (Advanced)**

To make the applet available for all users:

```bash
sudo mkdir -p /usr/share/cosmic/applets
sudo cp ./target/release/cosmic-applet /usr/share/cosmic/applets/network-monitor
sudo chmod +x /usr/share/cosmic/applets/network-monitor
```

**Note:** COSMIC applet registry integration is still evolving. If the applet doesn't appear in the "Add Applet" menu, use test mode (`--test-mode`) until the COSMIC Desktop applet discovery mechanism is finalized in your COSMIC version.

### 2. Remote Machine Setup (Per Machine)

#### Option A: Automated install script (recommended)

```bash
# Clone repo on remote machine
git clone https://github.com/USER/network-system-monitor.git
cd network-system-monitor

# Build release binary
cargo build --release -p nmd-service

# Run install script (will prompt for desktop IP)
sudo ./nmd-service/install-scripts/install.sh
```

The script will:
1. Prompt for desktop machine IP and UDP port
2. Generate local config at `/etc/nmd/config.toml`
3. Copy the HMAC secret key (you'll paste it when prompted)
4. Install binary to `/usr/local/bin/nmd-service`
5. Create and enable systemd service
6. Start the service

#### Option B: Manual installation

```bash
# Build binary
cargo build --release -p nmd-service

# Create config directory
sudo mkdir -p /etc/nmd

# Copy HMAC secret from desktop
sudo tee /etc/nmd/secret.key <<EOF
<paste the hex key from desktop>
EOF
sudo chmod 600 /etc/nmd/secret.key

# Create config file
sudo tee /etc/nmd/config.toml <<EOF
[service]
machine_name = "$(hostname)"
destination = "192.168.1.100:51057"  # Your desktop IP
interval_ms = 2000
hmac_secret_path = "/etc/nmd/secret.key"

[metrics]
cpu = true
memory = true
disk = true
network = true
uptime = true
gpu = true
temperature = true
EOF

# Install binary
sudo cp ./target/release/nmd-service /usr/local/bin/
sudo chmod 755 /usr/local/bin/nmd-service

# Create systemd unit
sudo tee /etc/systemd/system/nmd-service.service <<'EOF'
[Unit]
Description=Network Monitor Daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/nmd-service --config /etc/nmd/config.toml
Restart=always
RestartSec=5
User=nobody
Group=nogroup
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/etc/nmd
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable nmd-service
sudo systemctl start nmd-service
```

### 3. Verification

#### On remote machine

```bash
# Check service status
sudo systemctl status nmd-service

# View logs
sudo journalctl -u nmd-service -f

# Expected output:
#   "🔧 UDP sender initialized: buffer_len=XXX"
#   "📤 Sending metrics to 192.168.1.100:51057"
```

#### On desktop

```bash
# Check applet logs
./cosmic-applet --test-mode

# Expected output:
#   "🔐 Receiver HMAC secret loaded from /etc/nmd/secret.key"
#   "📡 Listening on 0.0.0.0:51057"
#   "✓ Packet received from pluto (sequence 42)"
```

Open the COSMIC panel and verify the applet shows metrics from all machines.

## Troubleshooting

### Remote machine not appearing on desktop

1. **Check HMAC secret matches**
   ```bash
   # On both machines
   cat /etc/nmd/secret.key
   ```
   The keys must be identical.

2. **Check network connectivity**
   ```bash
   # On remote machine, test UDP send
   echo "test" | nc -u 192.168.1.100 51057
   
   # On desktop, listen
   nc -lu 51057
   ```

3. **Check firewall**
   ```bash
   # On desktop, allow UDP 51057
   sudo ufw allow 51057/udp
   ```

4. **Check service logs**
   ```bash
   # Remote
   sudo journalctl -u nmd-service -n 50
   
   # Desktop (applet logs to stdout)
   ```

### HMAC verification failures

Symptom: Desktop logs show "HMAC verification failed"

Causes:
- Secret keys don't match (most common)
- Clock skew > 10 seconds between machines
- Corrupted UDP packets

Fix:
```bash
# Sync clocks
sudo ntpdate -s time.nist.gov

# Verify secrets match exactly
diff <(ssh remote-machine cat /etc/nmd/secret.key) /etc/nmd/secret.key
```

### High CPU usage on remote machines

Expected: < 1% CPU usage per nmd-service instance

If higher:
- Check metrics collection interval (default 2000ms is appropriate)
- Review which metrics are enabled
- Check for I/O bottlenecks (disk metrics on slow drives)

### Metrics not updating

1. Check service is running: `systemctl status nmd-service`
2. Verify config interval: `grep interval_ms /etc/nmd/config.toml`
3. Check applet is receiving packets: look for "Packet received" logs

## Security Notes

### HMAC Secret Key
- **Critical**: The secret key authenticates all metrics. Treat it like a password.
- Generated once on desktop, copied to all remote machines
- 32 bytes (256 bits) random hex
- Stored at `/etc/nmd/secret.key` with 0600 permissions

### Replay Protection
- Timestamp freshness: packets older than 10 seconds are rejected
- Sequence number: monotonic per machine, prevents replay attacks

### systemd Hardening
The service runs with restricted privileges:
- User: `nobody`
- `NoNewPrivileges=true`
- `ProtectSystem=strict`
- `ProtectHome=true`
- No network access required (outbound UDP only)

### Network Exposure
- Desktop listens on `0.0.0.0:51057` (all interfaces)
- Consider binding to LAN-only interface: edit config to `bind = "192.168.1.100:51057"`
- Firewall: allow UDP 51057 only from local network

## Uninstallation

### Remove from remote machine

```bash
sudo ./nmd-service/install-scripts/uninstall.sh
```

This will:
- Stop and disable the service
- Remove binary and systemd unit
- Optionally remove config directory

### Remove from desktop

```bash
# Stop applet (if running as systemd service)
# TODO: proper uninstall procedure

# Remove config
sudo rm -rf /etc/nmd
```

## Advanced Configuration

### Custom Metrics Interval

Edit `/etc/nmd/config.toml` on each remote machine:

```toml
[service]
interval_ms = 5000  # 5 seconds instead of 2
```

Then restart: `sudo systemctl restart nmd-service`

### Disable Specific Metrics

```toml
[metrics]
cpu = true
memory = true
disk = false        # Disable disk metrics
network = false     # Disable network metrics
uptime = true
gpu = false         # Disable GPU (useful for machines without GPU)
temperature = true
```

### Multiple Desktop Machines

You can send metrics to multiple desktops by running multiple nmd-service instances with different config files:

```bash
# /etc/nmd/config-desktop1.toml
[service]
destination = "192.168.1.100:51057"

# /etc/nmd/config-desktop2.toml
[service]
destination = "192.168.1.101:51057"

# Create separate systemd units (nmd-service@desktop1.service, nmd-service@desktop2.service)
```

## Next Steps

1. Add more remote machines (repeat step 2 for each)
2. Configure which metrics to display (use applet config UI)
3. Customize panel display order
4. Set up per-machine thresholds (TODO: feature not yet implemented)

## Support

- Report issues: https://github.com/USER/REPO/issues
- View logs: `journalctl -u nmd-service -f` (remote), `./cosmic-applet --test-mode` (desktop)
- Configuration reference: See `config.toml` examples in this guide
