# 5-Sender Stress Test

**Purpose:** Validate performance monitoring (Item 7.2) and packet loss detection (Item 7.3) under concurrent load.

---

## Overview

This test simulates 5 remote machines sending metrics to one receiver:
- **5 Senders:** All running on localhost, named `a`, `b`, `c`, `d`, `e`
- **1 Receiver:** cosmic-applet listening on port 51057
- **Refresh Rate:** 1 second (5 packets/sec total)
- **Pairing:** Each machine pairs with the receiver via TCP to receive X25519 pubkey

**What It Tests:**
- ✅ Concurrent UDP packet handling
- ✅ Performance monitoring logs (Item 7.2: >50ms warnings)
- ✅ Packet loss detection (Item 7.3: sequence gap warnings)
- ✅ Session tracking with sender_session_id (SEC-03)
- ✅ System stability under continuous load

---

## Quick Start

### 1. Setup and Start Senders

```bash
./test-5-senders.sh setup
```

**Output:**
```
=== Setting up 5-sender test environment ===
✓ Config files created for machines a-e (no shared secret needed)
✓ Created config for machine 'a'
✓ Created config for machine 'b'
✓ Created config for machine 'c'
✓ Created config for machine 'd'
✓ Created config for machine 'e'
✓ Created receiver config

=== Setup complete ===

=== Starting 5 sender processes ===
✓ Started sender 'a' (PID 12345) → test-env/5-senders/logs/sender-a.log
✓ Started sender 'b' (PID 12346) → test-env/5-senders/logs/sender-b.log
✓ Started sender 'c' (PID 12347) → test-env/5-senders/logs/sender-c.log
✓ Started sender 'd' (PID 12348) → test-env/5-senders/logs/sender-d.log
✓ Started sender 'e' (PID 12349) → test-env/5-senders/logs/sender-e.log

All 5 senders running!
```

### 2. Start Receiver (Separate Terminal)

**Option A: Using script (recommended)**
```bash
./run-5-receiver.sh
```

**Option B: Automated via test script**
```bash
./test-5-senders.sh receiver
```

**Option C: Manual**
```bash
RUST_LOG=debug cargo run --bin cosmic-applet -- test-env/5-senders/receiver-config.toml
```

### 3. Monitor Logs in Real-Time

```bash
./test-5-senders.sh monitor
```

**Expected Output:**
```
=== Monitoring logs (Ctrl+C to stop) ===

Performance warnings (>50ms collectors):
[DEBUG] nmd_service::metrics_aggregator: Metrics aggregation completed in 12ms
[DEBUG] nmd_service::metrics_aggregator: Metrics aggregation completed in 8ms
[WARN]  nmd_service::metrics_aggregator: GPU collection took 87ms (threshold: 50ms)

Receiver events:
[INFO]  cosmic_applet::network::udp_receiver: 🆕 New session detected: machine 'a' session '01234567'
[INFO]  cosmic_applet::network::udp_receiver: 🆕 New session detected: machine 'b' session '89abcdef'
[WARN]  cosmic_applet::network::udp_receiver: 📉 Packet loss detected: machine 'c' session 'fedcba98' — lost 2 packet(s) (seq 42-43)
```

### 4. View Performance Summary

```bash
./test-5-senders.sh summary
```

**Expected Output:**
```
=== Performance Summary ===

Machine 'a':
  Total aggregations: 120
  Slow collectors (>50ms): 3
  Average aggregation time: 11.2ms

Machine 'b':
  Total aggregations: 120
  Slow collectors (>50ms): 0
  Average aggregation time: 9.8ms

Machine 'c':
  Total aggregations: 120
  Slow collectors (>50ms): 1
  Average aggregation time: 10.5ms

Machine 'd':
  Total aggregations: 120
  Slow collectors (>50ms): 2
  Average aggregation time: 12.1ms

Machine 'e':
  Total aggregations: 120
  Slow collectors (>50ms): 0
  Average aggregation time: 9.3ms

Receiver:
  New sessions detected: 5
  Packet loss events: 0
  Replay attempts: 0
```

### 5. Cleanup

```bash
./test-5-senders.sh cleanup
```

---

## Test Duration Recommendations

### Quick Smoke Test (30 seconds)
```bash
./test-5-senders.sh setup
./test-5-senders.sh receiver
sleep 30
./test-5-senders.sh summary
./test-5-senders.sh cleanup
```

**Expected Results:**
- ~150 total packets (5 senders × 30 seconds)
- All 5 sessions detected
- 0 packet loss (localhost UDP is reliable)
- Average aggregation time < 20ms

### Stress Test (5 minutes)
```bash
./test-5-senders.sh setup
./test-5-senders.sh receiver
sleep 300
./test-5-senders.sh summary
./test-5-senders.sh cleanup
```

**Expected Results:**
- ~1,500 total packets (5 senders × 5 minutes)
- Consistent performance (no degradation over time)
- Possible slow collector warnings if GPU/disk access spikes
- Memory usage stable (no leaks)

### Long-Running Stability (1 hour+)
```bash
./test-5-senders.sh setup
./test-5-senders.sh receiver

# Monitor resource usage
watch -n 10 'ps aux | grep -E "nmd-service|cosmic-applet" | grep -v grep'

# After 1+ hours
./test-5-senders.sh summary
./test-5-senders.sh cleanup
```

**What to Monitor:**
- Memory usage should remain flat (no gradual increase)
- CPU usage should stay low (~1-5% per sender)
- No sustained warnings about slow collectors

---

## Log File Locations

All logs are stored in `test-env/5-senders/logs/`:

```
test-env/5-senders/
# No shared key needed - each machine pairs with receiver via TCP
├── config-a.toml                 # Sender configs
├── config-b.toml
├── config-c.toml
├── config-d.toml
├── config-e.toml
├── receiver-config.toml          # Receiver config
├── receiver.log                  # Receiver output (if using ./test-5-senders.sh receiver)
└── logs/
    ├── sender-a.log              # Individual sender logs
    ├── sender-b.log
    ├── sender-c.log
    ├── sender-d.log
    └── sender-e.log
```

---

## What to Look For

### ✅ Good Signs

**Performance Monitoring (Item 7.2):**
```
[DEBUG] Metrics aggregation completed in 10ms
[DEBUG] Metrics aggregation completed in 12ms
[DEBUG] Metrics aggregation completed in 9ms
```

**Session Detection (SEC-03):**
```
[INFO] 🆕 New session detected: machine 'a' session '01234567'
[INFO] 🆕 New session detected: machine 'b' session '89abcdef'
```

**Clean Operation:**
- No packet loss warnings
- No replay detection warnings
- Consistent aggregation times

### ⚠️ Warning Signs (May Be Normal)

**Occasional Slow Collectors:**
```
[WARN] GPU collection took 87ms (threshold: 50ms)
[WARN] Disk collection took 62ms (threshold: 50ms)
```

**Reason:** Disk/GPU operations can spike due to system activity. Occasional warnings are acceptable.

**Action:** If warnings are frequent (>10% of aggregations), investigate:
- Is the disk under heavy I/O load?
- Is GPU running compute workload?
- Are sensors slow to read?

### 🚨 Red Flags

**Packet Loss on Localhost:**
```
[WARN] 📉 Packet loss detected: machine 'c' session 'fedcba98' — lost 5 packet(s)
```

**Reason:** Shouldn't happen on localhost UDP. Indicates receiver overload or kernel buffer issues.

**Action:** Check receiver CPU usage, increase socket buffer sizes, or reduce sender refresh rate.

**Sustained Slow Aggregation:**
```
[WARN] Total metrics aggregation took 123ms (threshold: 50ms)
[WARN] Total metrics aggregation took 156ms (threshold: 50ms)
```

**Reason:** System is struggling to collect metrics fast enough for 1-second refresh.

**Action:** Increase `refresh_interval_secs` in configs, or investigate which collector is slow.

---

## Interpreting Results

### Performance Baseline (Expected on Modern Hardware)

| Metric | Expected Value | Threshold |
|--------|----------------|-----------|
| Average aggregation time | 8-15ms | 50ms |
| Slow collector warnings | <5% of runs | >10% problematic |
| Packet loss (localhost) | 0 | Any loss is unusual |
| Memory per sender | ~10-20MB | Flat over time |
| CPU per sender | 1-3% | <5% sustained |

### What Each Log Line Means

**Sender Logs (Item 7.2):**
```
[DEBUG] CPU collection took 2ms
[DEBUG] Network collection took 1ms
[DEBUG] Temperature collection took 5ms
[DEBUG] Memory collection took 3ms
[DEBUG] Disk collection took 8ms
[DEBUG] Uptime collection took 0ms
[DEBUG] GPU collection took 12ms
[DEBUG] Metrics aggregation completed in 31ms
```

**Breakdown:** Total = sum of all collectors. If total >50ms, warning is logged.

**Receiver Logs (Item 7.3):**
```
[INFO] 🆕 New session detected: machine 'a' session '01234567'
```
→ First packet from this (machine_id, sender_session_id) tuple. Normal on startup.

```
[WARN] 📉 Packet loss detected: machine 'c' session 'fedcba98' — lost 2 packet(s) (seq 42-43)
```
→ Receiver saw sequence jump from 41 → 44, meaning packets 42-43 were lost. Check network reliability.

```
[WARN] Replay detected: machine 'e' session 'abcd1234' seq 100 <= last 100
```
→ Duplicate or out-of-order packet. Could indicate network issue or attack attempt.

---

## Troubleshooting

### Senders Won't Start

**Error:** `Failed to bind UDP socket: Address already in use`

**Fix:** Each sender sends *from* a random ephemeral port, so this shouldn't happen. Check if another process is holding port 51057 (the receiver port).

### Receiver Shows No Sessions

**Symptom:** `./test-5-senders.sh summary` shows 0 new sessions.

**Diagnosis:**
1. Check receiver is running: `ps aux | grep cosmic-applet`
2. Check sender logs for errors: `tail test-env/5-senders/logs/sender-*.log`
3. Verify configs point to correct port: `grep port test-env/5-senders/config-*.toml`

**Fix:** Restart both senders and receiver with verbose logging:
```bash
./test-5-senders.sh cleanup
RUST_LOG=debug ./test-5-senders.sh setup
RUST_LOG=debug ./test-5-senders.sh receiver
```

### Constant Packet Loss Warnings

**Symptom:** Every packet shows loss warning.

**Diagnosis:** Receiver UDP buffer may be full.

**Fix:** Increase kernel UDP buffer size:
```bash
sudo sysctl -w net.core.rmem_max=8388608
sudo sysctl -w net.core.rmem_default=8388608
```

### High CPU Usage

**Symptom:** Senders using >10% CPU each.

**Diagnosis:** Metrics collection is too expensive for 1-second refresh.

**Fix:** Increase refresh interval in configs:
```bash
# Edit all config files
sed -i 's/refresh_interval_secs = 1/refresh_interval_secs = 2/' test-env/5-senders/config-*.toml
./test-5-senders.sh cleanup
./test-5-senders.sh setup
```

---

## Validation Checklist

After running the test, verify:

- [ ] All 5 senders started successfully
- [ ] Receiver detected 5 new sessions
- [ ] No packet loss warnings on localhost
- [ ] Average aggregation time < 20ms
- [ ] No slow collector warnings (or <5% of runs)
- [ ] Memory usage flat over test duration
- [ ] CPU usage reasonable (<5% per sender)
- [ ] Summary shows expected packet counts (~refresh_rate × duration × 5 machines)

---

## Next Steps

### If Test Passes ✅

System is production-ready:
1. Deploy to real network (separate machines)
2. Monitor for packet loss over WiFi/LAN
3. Adjust refresh rates based on observed performance

### If Test Fails ❌

Investigate issues:
1. Review slow collector warnings → optimize those metrics
2. Check packet loss patterns → network or buffer tuning
3. Monitor memory growth → potential leak (run Item 7.4: valgrind)
4. High CPU → reduce refresh rate or optimize collectors

---

## Related Documentation

- [PRODUCTION-READINESS.md](PRODUCTION-READINESS.md) — Full production checklist (Items 1-7)
- [Item 7.2 Implementation](.agentreports/observability/2026-08-27-items-7.1-7.3-implementation.md) — Performance monitoring details
- [Item 7.3 Implementation](.agentreports/observability/2026-08-27-items-7.1-7.3-implementation.md) — Packet loss detection logic
- [TEST-GUIDE.md](TEST-GUIDE.md) — General testing documentation
- [DEPLOYMENT.md](DEPLOYMENT.md) — Production deployment instructions
