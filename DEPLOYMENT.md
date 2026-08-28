# Deployment Guide

Complete guide for deploying Network System Monitor across your home network.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│ Desktop Machine (COSMIC Desktop)                                │
│  - cosmic-applet (panel widget + config UI + pairing manager)   │
│  - ChaCha20-Poly1305 receiver on port 51057                     │
│  - TOFU pairing: ~/.config/cosmic-applet/pairing.toml           │
└─────────────────────────────────────────────────────────────────┘
                              ▲
                              │ UDP packets every 1s
                              │ (ChaCha20-Poly1305 AEAD)
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

#### Create config file

The cosmic-applet loads its configuration from `~/.config/cosmic-applet/config.toml` (or `config.toml` in the working directory during `--test` mode):

```bash
mkdir -p ~/.config/cosmic-applet

sudo tee ~/.config/cosmic-applet/config.toml <<EOF
udp_port = 51057

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
EOF
```

**Note:** No shared secret key is needed at setup time. The first connection from each remote machine triggers an automatic pairing request in the applet UI.

#### Install and run applet

**Option A: Test Mode (Development)**

For quick testing without full installation:

```bash
./target/release/cosmic-applet --test
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

**Note:** COSMIC applet registry integration is still evolving. If the applet doesn't appear in the "Add Applet" menu, use test mode (`--test`) until the COSMIC Desktop applet discovery mechanism is finalized in your COSMIC version.

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
3. Install binary to `/usr/local/bin/nmd-service`
4. Create and enable systemd service
5. Start the service

#### Option B: Manual installation

```bash
# Build binary
cargo build --release -p nmd-service

# Create config directory
sudo mkdir -p /etc/nmd

# Create config file (no secret key needed)
sudo tee /etc/nmd/config.toml <<EOF
host = "192.168.1.100"         # Your desktop IP
port = 51057
refresh_interval_secs = 1
machine_id = "$(hostname)"     # Unique name for this machine

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

### 3. Pairing Flow (First Connection)

When the first UDP packet arrives from a remote machine:

1. The desktop applet detects an unknown sender
2. A pairing request appears in the applet's dropdown menu
3. The UI shows:
   - Machine ID (from `machine_id` field in config)
   - Sender IP address
   - Accept/Deny buttons

**Accept:** The receiver generates a ChaCha20 shared key via ECDH and stores it in `~/.config/cosmic-applet/pairing.toml`

**Deny:** The packet is dropped, no pairing entry created

**Pre-production note:** Currently all machines use the same `TEMP_SHARED_KEY = [0x42; 32]` placeholder. Per-machine ECDH keys are wired in but not yet fully enabled (sender pubkey field is `[0u8; 32]` placeholder).

## Verification

### On remote machine

```bash
# Check service status
sudo systemctl status nmd-service

# View logs
sudo journalctl -u nmd-service -f

# Expected output:
#   "Loaded config from /etc/nmd/config.toml — host=192.168.1.100, port=51057, refresh_interval=1s"
#   "UDP sender initialized: dest=192.168.1.100:51057"
#   "Sending metrics to 192.168.1.100:51057 (machine_id=pluto, interval=1s)"
```

### On desktop

```bash
# Check applet logs
./target/release/cosmic-applet --test

# Expected output:
#   "Loaded config from ~/.config/cosmic-applet/config.toml"
#   "Listening on 0.0.0.0:51057"
#   "🔔 Received pairing request from unpaired machine: pluto (host: 192.168.1.100)"
#   "✅ Pairing accepted for machine: pluto"
```

Open the COSMIC panel and verify the applet shows metrics from all machines.

## Troubleshooting

### Remote machine not appearing on desktop

1. **Check network connectivity**
   ```bash
   # On remote machine, test UDP send
   echo "test" | nc -u 192.168.1.100 51057
   
   # On desktop, listen
   nc -lu 51057
   ```

2. **Check firewall**
   ```bash
   # On desktop, allow UDP 51057
   sudo ufw allow 51057/udp
   ```

3. **Check service logs**
   ```bash
   # Remote
   sudo journalctl -u nmd-service -n 50
   
   # Desktop (applet logs to stdout)
   ```

4. **Verify config file format**
   ```bash
   # Check remote config has correct fields
   cat /etc/nmd/config.toml
   # Should contain: host, port, refresh_interval_secs, machine_id (NOT hmac_secret_path)
   ```

### High CPU usage on remote machines

Expected: < 1% CPU usage per nmd-service instance

If higher:
- Check metrics collection interval (`refresh_interval_secs` in config — default 1s is appropriate)
- Review which metrics are enabled
- Check for I/O bottlenecks (disk metrics on slow drives)

### Metrics not updating

1. Check service is running: `systemctl status nmd-service`
2. Verify config interval: `grep refresh_interval_secs /etc/nmd/config.toml`
3. Check applet is receiving packets: look for "Packet received" logs
4. Verify pairing was accepted: check `~/.config/cosmic-applet/pairing.toml`

## Security Notes

### ChaCha20-Poly1305 AEAD Encryption
- Confidentiality + authenticity in one operation (no separate HMAC)
- 12-byte nonce + ciphertext + 16-byte Poly1305 tag per packet
- Tampered packets are rejected during decryption

### TOFU Pairing System
- First connection from unknown sender triggers UI prompt
- Accepted pairings store ECDH-derived shared key in `~/.config/cosmic-applet/pairing.toml`
- Replay protection: timestamp freshness (< 10s) + monotonic sequence numbers per session

### systemd Hardening
The service runs with restricted privileges:
- User: `nobody`
- `NoNewPrivileges=true`
- `ProtectSystem=strict`
- `ProtectHome=true`
- No network access required (outbound UDP only)

### Network Exposure
- Desktop listens on `0.0.0.0:51057` (all interfaces)
- Consider binding to LAN-only interface by editing config
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
rm -rf ~/.config/cosmic-applet/pairing.toml  # pairing data
```

## Advanced Configuration

### Custom Metrics Interval

Edit `/etc/nmd/config.toml` on each remote machine:

```toml
refresh_interval_secs = 5  # 5 seconds instead of 1
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
host = "192.168.1.100"
port = 51057

# /etc/nmd/config-desktop2.toml
host = "192.168.1.101"
port = 51057

# Create separate systemd units (nmd-service@desktop1.service, nmd-service@desktop2.service)
```

## Next Steps

1. Add more remote machines (repeat step 2 for each)
2. Configure which metrics to display (use applet config UI)
3. Customize panel display order
4. Set up per-machine thresholds (TODO: feature not yet implemented)

## Support

- Report issues: https://github.com/USER/REPO/issues
- View logs: `journalctl -u nmd-service -f` (remote), `./cosmic-applet --test` (desktop)
- Configuration reference: See CONFIGURATION.md
