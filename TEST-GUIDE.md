# Network System Monitor - Test Guide

## Quick Start

The test environment simulates a complete network monitoring setup on localhost:

```bash
# 1. Initialize test environment (one time)
./test-debug.sh

# 2. Terminal 1 - Start the metrics sender
./run-sender.sh

# 3. Terminal 2 - Start the applet receiver (or use COSMIC panel)
./run-receiver.sh
```

## What to Look For

### Debug Output (nmd-service sender)
```
INFO  nmd_service — nmd-service starting up
INFO  nmd_service — Loaded config — host=127.0.0.1, port=51057, interval=2000ms
DEBUG nmd_service — Sent metrics — seq=1, cpu=12.5%, mem=45.3%
DEBUG nmd_service — Sent metrics — seq=2, cpu=13.1%, mem=45.4%
```

### Debug Output (cosmic-applet receiver)
```
DEBUG cosmic_applet::network — Received packet from 127.0.0.1 (342 bytes)
DEBUG cosmic_applet::network — HMAC verification passed
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

### Test Secret Key
Location: `test-env/etc/nmd/secret.key`
- 32-byte hex string for HMAC-SHA256
- Shared between sender and receiver

### nmd-service Config
Location: `test-env/nmd-config.toml`
```toml
host = "127.0.0.1"
port = 51057
interval_ms = 2000
machine_id = "test-machine"
hmac_secret_path = "./test-env/etc/nmd/secret.key"
```

### cosmic-applet Config
Location: `test-env/applet-config.toml`
```toml
[udp_receiver]
port = 51057
hmac_secret_path = "./test-env/etc/nmd/secret.key"

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

### "HMAC verification failed"
- Check that both sender and receiver use the same secret key
- Verify secret key is exactly 32 bytes
- Check file permissions (should be 600)

### "No data received"
- Verify port 51057 is not in use: `netstat -an | grep 51057`
- Check firewall rules (should allow localhost traffic)
- Look for errors in sender debug output

### "Connection refused"
- Start the receiver (cosmic-applet) before the sender (nmd-service)
- UDP is connectionless but the receiver must be listening

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

1. Install nmd-service on remote machines: `nmd-service/install-scripts/install.sh`
2. Configure cosmic-applet with real machine IPs in `config.toml`
3. Set up systemd service for nmd-service on each machine
4. Monitor multiple machines from single desktop panel
