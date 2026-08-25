# nmd-service Install Scripts

Scripts for installing the Network System Monitor systemd service on remote machines (Pluto, Spark, etc.).

## Quick Start

```bash
# On each remote machine:
./install.sh              # Installs binary + config + secret key + systemd unit
systemctl status nmd      # Verify it's running
journalctl -u nmd -f     # Follow logs in real-time
```

## Files

| File | Purpose |
|------|---------|
| `install.sh` | One-command install: creates `/etc/nmd/`, generates secret key, writes config.toml, copies binary, installs systemd unit, starts service |
| `generate-certs.sh` | *(Deferred)* Future cert generation for mTLS fallback — currently uses HMAC pre-shared keys instead per Worf's Phase 1A security analysis |

## Configuration

After install, edit `/etc/nmd/config.toml`:

```toml
host = "192.168.1.10"   # Desktop applet IP address (REQUIRED EDIT)
port = 51057            # Desktop UDP listener port
interval_ms = 2000      # Send interval in milliseconds (default: every 2s per spec)
machine_id = ""          # Auto-detected from hostname if left empty
hmac_secret_path = "/etc/nmd/secret.key"
```

## Security Notes

- **HMAC-SHA256 Authentication**: Each packet is signed with a pre-shared key at `/etc/nmd/secret.key` (0600 permissions, 32 bytes). Copy this same key to your desktop applet's config for verification.
- **Replay Protection**: Packets include timestamp (< 10s freshness) + monotonic sequence number per machine_id.
- **Least Privilege**: Service runs as `nobody:nogroup` with systemd hardening (`NoNewPrivileges`, `ProtectSystem=strict`).

## Uninstall

```bash
systemctl stop nmd
systemctl disable nmd
rm /etc/systemd/system/nmd.service
rm -rf /etc/nmd/
rm /usr/local/bin/nmd-service
systemctl daemon-reload
```