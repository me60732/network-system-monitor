# Deployment Guide

Complete guide for deploying Network System Monitor across your home network.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│ Desktop Machine (COSMIC Desktop)                                │
│  - cosmic-applet (panel widget + machine list UI + config)      │
│  - UDP receiver on port 51057                                   │
│  - Per-machine sensor config via gear icon in machine detail    │
│  - Global settings: value size, monospace font, panel spacing   │
└─────────────────────────────────────────────────────────────────┘
                              ▲
                              │ UDP packets every 1s
                              │ (ChaCha20-Poly1305 AEAD + TOFU pairing)
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
┌───────▼────────┐    ┌───────▼────────┐    ┌───────▼────────┐
│ Remote Machine │    │ Remote Machine │    │ Remote Machine │
│ (Pluto)        │    │ (Spark)        │    │ ...            │
│ nmd            │    │ nmd            │    │ nmd            │
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

# Per-machine sensor configuration (what shows in panel row)
sensor_config = {
  cpu_chart_visible = true,
  memory_chart_visible = true,
  disk_chart_visible = false,  # Disk not shown in row (configured per-machine)
  network_chart_visible = true,
  gpu_load_chart_visible = true,
  gpu_vram_chart_visible = true,
  temperature_chart_visible = true,
}

# Global settings apply to ALL machines (configured via Settings UI):
# - value_size: font size for metric values
# - monospace_font: use monospace font for values
# - panel_spacing: spacing between sensors in row
# - content_order: order of sensors left-to-right in panel
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

### 2. Local Machine Setup (Testing on Same Machine)

The applet and nmd can both run on the same machine for testing:

```bash
# Build both binaries
cargo build --release -p nmd-service -p cosmic-applet

# Create sender config at ~/.config/nmd/config.toml
cat > ~/.config/nmd/config.toml <<EOF
host = "127.0.0.1"
port = 51057
refresh_interval_secs = 1
machine_id = "localhost-test"
EOF

# Run sender (in background)
./target/release/nmd-service --config ~/.config/nmd/config.toml &

# Run receiver in test mode
./target/release/cosmic-applet --test
```

The sender's `host` should be `127.0.0.1` for local testing.

### 3. Remote Machine Setup (Per Machine)

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
4. Create and enable nmd systemd service
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
sudo tee /etc/systemd/system/nmd.service <<'EOF'
[Unit]
Description=Network System Monitor — metrics sender
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/nmd-service --config /etc/nmd/config.toml
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=nmd

# Run as dedicated unprivileged user
User=nmd
Group=nmd
Environment=HOME=/var/lib/nmd
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
sudo systemctl enable nmd
sudo systemctl start nmd
```

### 4. Pairing Flow (First Connection)

When the first UDP packet arrives from a remote machine:

1. The desktop applet detects an unknown sender via TOFU pairing system
2. A pairing request appears in the applet's UI dropdown menu
3. The UI shows:
   - Machine ID (from `machine_id` field in nmd config)
   - Sender IP address
   - Accept/Deny dropdown

**Accept:** The receiver generates an ECDH-derived shared key and stores it in `~/.config/cosmic-applet/pairing.toml`

**Deny:** The packet is dropped, no pairing entry created

**Automatic sender setup:** After accepting the pairing:
   - The sender automatically receives the receiver's X25519 pubkey via TCP
   - The sender stores it in `/etc/nmd/config.toml` as `receiver_pubkey = "<hex>"`
   - Subsequent packets use ECDH-derived keys (fully encrypted end-to-end)

**Manual setup (if TCP pairing unavailable):**
   - Copy the receiver's X25519 pubkey from applet Settings → General
   - Add it to sender config: `receiver_pubkey = "<hex>"` in `/etc/nmd/config.toml`
   - Restart: `systemctl restart nmd`



## Verification

### On remote machine

```bash
# Check service status
sudo systemctl status nmd

# View logs
sudo journalctl -u nmd -f

# Expected output:
#   "Loaded config from /etc/nmd/config.toml — host=192.168.1.100, port=51057, refresh_interval_secs=1"
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
   sudo journalctl -u nmd -n 50
   
   # Desktop (applet logs to stdout)
   ```

4. **Verify config file format**
   ```bash
   # Check remote config has correct fields
   cat /etc/nmd/config.toml
   # Should contain: host, port, refresh_interval_secs, machine_id (NOT hmac_secret_path)
   ```

### High CPU usage on remote machines

Expected: < 1% CPU usage per nmd instance

If higher:
- Check metrics collection interval (`refresh_interval_secs` in `/etc/nmd/config.toml` — default 1s is appropriate)
- Review which metrics are enabled in `[metrics]` section
- Check for I/O bottlenecks (disk metrics on slow drives)

### Metrics not updating

1. Check service is running: `systemctl status nmd`
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
The nmd service runs with restricted privileges:
- User: `nmd` (dedicated system user)
- `NoNewPrivileges=true`
- `ProtectSystem=strict`
- `ProtectHome=read-only`
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
- Stop and disable nmd service
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

### Custom Refresh Rate (Per-Machine)

Edit `/etc/nmd/config.toml` on each remote machine:

```toml
refresh_interval_secs = 5  # 5 seconds instead of 1
```

Then restart: `sudo systemctl restart nmd`

**Note:** Refresh rate is configured per-machine via nmd config, NOT in the applet's global settings UI.

### Disable Specific Metrics Per Machine

Edit `/etc/nmd/config.toml` on each remote machine:

```toml
[metrics]
cpu = true
memory = true
disk = false        # Disable disk metrics collection
network = false     # Disable network metrics collection
uptime = true
gpu = false         # Disable GPU (useful for machines without GPU)
temperature = true
```

**Note:** Metric display in the panel row is controlled per-machine via the `sensor_config` section in MachineConfig (configured via gear icon in machine detail view).

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
2. Configure per-machine sensor display via gear icon in machine detail view
3. Global settings: value size, monospace font, panel spacing, content order

## Support

- Report issues: https://github.com/USER/REPO/issues
- View logs: `journalctl -u nmd -f` (remote), `./cosmic-applet --test` (desktop)
- Configuration reference: See CONFIGURATION.md
