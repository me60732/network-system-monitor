# nmd-service

Remote systemd service for the **Network System Monitor**. Runs on each Linux machine (Pluto, Spark, etc.) to collect system metrics and push them via UDP with rkyv serialization + HMAC-SHA256 authentication to the desktop Cosmic applet.

## Overview

```
┌─────────────────────────────────────────────────────────┐
│  Remote Machine (nmd-service — this crate)              │
│                                                         │
│  metrics-core::collect_all() → MetricPacket             │
│        ↓ rkyv encode + HMAC-SHA256 sign                  │
│        ↓ UDP push (every 2s)                            │
└───────────────────────────→ ┌─────────────────────────┐
                              │ Desktop Cosmic Applet    │
                              │ (listens on :51057)      │
                              └─────────────────────────┘
```

## Architecture

| Module | Responsibility |
|--------|---------------|
| `main.rs` | systemd entry point, CLI arg parsing (`--config`), SIGTERM/SIGINT graceful shutdown, main loop |
| `config.rs` | TOML config loading (host, port, interval_ms, machine_id), secret key file I/O from `/etc/nmd/secret.key` |
| `packet.rs` | `MetricPacket` struct with rkyv::Archive derive + HMAC fields (timestamp, sequence, hmac_tag) |
| `udp_sender.rs` | UDP transmission with HMAC-SHA256 authentication, atomic sequence counter |
| `metrics_aggregator.rs` | Calls `metrics_core::collect_all()`, packs results into flat MetricPacket format |
| `systemd_unit.rs` | systemd unit file constants + install/uninstall helpers for remote deployment |

## Security (Worf Phase 1A)

- **HMAC-SHA256** authentication with pre-shared key stored at `/etc/nmd/secret.key` (0600, 32 bytes)
- **Replay protection**: timestamp freshness (< 10s old) + monotonic sequence number per machine_id
- **Least privilege**: runs as `nobody:nogroup`, systemd hardening directives (`NoNewPrivileges`, `ProtectSystem=strict`)

## Build & Install

```bash
# From workspace root:
cargo build --release -p nmd-service

# On remote machine:
./nmd-service/install-scripts/install.sh    # One-command install + start
systemctl status nmd                         # Verify running
journalctl -u nmd -f                        # Follow logs
```

## Development Phases

This crate is currently in **Phase 1A (Scaffolding)** — all modules compile with stub implementations. Real logic will be filled in by ensign agents after Beverly's testing phase review of `metrics-core`.