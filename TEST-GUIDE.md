# Network System Monitor - Test Guide

> **Status**: Outdated - Multi-sender stress test scripts moved to `test-scripts/` folder.
> Last updated: 2026-09-02

This guide describes legacy multi-sender stress testing. For current single-machine testing:

```bash
# Build both binaries
cargo build --release -p nmd-service -p cosmic-applet

# Terminal 1 - Start sender (config at ~/.config/nmd/config.toml)
./target/release/nmd-service --config ~/.config/nmd/config.toml &

# Terminal 2 - Start applet in test mode
./target/release/cosmic-applet --test
```

## What to Look For

### Debug Output (nmd-service sender)
```
INFO  nmd_service — nmd-service starting up
INFO  nmd_service — Loaded config — host=127.0.0.1, port=51057, refresh_interval_secs=1
DEBUG nmd_service — Sent metrics — seq=1, cpu=12.5%, mem=45.3%
DEBUG nmd_service — Sent metrics — seq=2, cpu=13.1%, mem=45.4%
```

### Debug Output (cosmic-applet receiver)
```
DEBUG cosmic_applet::network — Received packet from 127.0.0.1 (342 bytes)
DEBUG cosmic_applet::network — ChaCha20-Poly1305 AEAD decryption and tag verification passed
DEBUG cosmic_applet::network — Updated machine: test-machine
```

## Testing Adaptive Unit Scaling

The adaptive unit scaling automatically switches between KB/s, MB/s, and GB/s based on throughput:

- **KB/s** when < 100 KB/s: "45.3 KB/s"
- **MB/s** when >= 100 KB/s: "120.5 MB/s"  
- **GB/s** when >= 100 MB/s: "2.3 GB/s"

### Generate Test Traffic

To test the scaling with real network activity:

```bash
# In another terminal, generate some network load
# Large file download (will show MB/s or GB/s)
wget http://speedtest.tele2.net/100MB.zip -O /dev/null

# Or continuous traffic
while true; do curl -s http://example.com > /dev/null; sleep 0.1; done
```

Watch the applet panel and machine detail view - the units should automatically adjust to keep numbers compact and readable.

## Verifying All Features

1. **Ring Charts** - CPU, Memory, GPU should show percentage rings with auto-formatted text
2. **Network Throughput** - Download/upload arrows with adaptive units (KB/s → MB/s → GB/s)
3. **Disk I/O** - Write/Read with adaptive units (no rings, text-only)
4. **Temperature** - CPU temp in ring chart with custom °C text
5. **Uptime** - Formatted duration string

## Configuration Files

### No shared secret key needed - ChaCha20-Poly1305 AEAD uses per-machine ECDH-derived keys

### nmd-service Config
Location: `test-env/nmd-config.toml`
```toml
host = "127.0.0.1"
port = 51057
refresh_interval_secs = 1
machine_id = "test-machine"
# receiver_pubkey is set automatically via TCP pairing on first start
# receiver_pubkey = "<64-char hex X25519 public key from applet Settings → General>"
```

### cosmic-applet Config
Location: `test-env/applet-config.toml`
```toml
# No hmac_secret_path needed - ChaCha20-Poly1305 AEAD uses per-machine ECDH-derived keys

[[machines]]
name = "test-machine"
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
```

## Troubleshooting

### "Decryption failed" or "Tag verification failed"
- Ensure receiver's X25519 pubkey is correctly set in sender config (`receiver_pubkey = "<hex>"`)
- Verify pairing was accepted: check `~/.config/cosmic-applet/pairing.toml`

### "No data received"
- Verify port 51057 is not in use: `netstat -an | grep 51057`
- Check firewall rules (should allow localhost traffic)
- Look for errors in sender debug output

### "Connection refused" during TCP pairing
- Start the receiver (cosmic-applet) before the sender (nmd-service)
- The sender initiates a TCP connection to receive the receiver's X25519 pubkey

### Metrics not updating
- Check sender is running (`ps aux | grep nmd-service`)
- Verify interval_ms is set (default 2000ms = 2 seconds)
- Look for rate limiting or throttling in logs

## Cleaning Up

```bash
# Stop both processes (Ctrl-C in each terminal)

# Remove test environment
rm -rf test-env/

# Restore original applet config (if backed up)
mv ~/.config/com.system-76.CosmicApplet/config.toml.backup \
   ~/.config/com.system-76.CosmicApplet/config.toml
```

## Next Steps

Once local testing is complete:

1. Install nmd on remote machines: `nmd-service/install-scripts/install.sh`
2. Configure cosmic-applet with real machine IPs in config.toml
3. Per-machine sensor configuration via gear icon in machine detail view
4. Monitor multiple machines from single desktop panel
