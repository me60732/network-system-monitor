# nmd-service

Remote systemd service for the **Network System Monitor**. Runs on each Linux machine (Pluto, Spark, etc.) to collect system metrics and push them via UDP with rkyv serialization + ChaCha20-Poly1305 AEAD encryption to the desktop Cosmic applet.

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
| `config.rs` | TOML config loading (host, port, refresh_interval_secs, machine_id), keypair I/O from `/var/lib/nmd/.config/nmd/keypair.key` |
| `packet.rs` | `MetricPacket` struct with rkyv::Archive derive + ChaCha20-Poly1305 fields (timestamp, sequence, nonce, ciphertext) |
| `udp_sender.rs` | UDP transmission with ChaCha20-Poly1305 AEAD encryption, atomic sequence counter per session |
| `metrics_aggregator.rs` | Calls `metrics_core::collect_all()`, packs results into flat MetricPacket format |
| `systemd_unit.rs` | systemd unit file constants + install/uninstall helpers for remote deployment |
| `crypto.rs` | ECDH key derivation, TOFU pairing TCP connection to receiver |
| `pairing_client.rs` | TCP client for initial pairing handshake with cosmic-applet |
| `receiver_pubkey_manager.rs` | Manages `receiver_pubkey` in config after pairing acceptance |

## Security (Worf Phase 1A)

- **ChaCha20-Poly1305 AEAD** encryption with ECDH-derived per-machine shared keys
- **Pairing flow**: TCP connection to receiver on first start, receives receiver's X25519 pubkey, stores in config as `receiver_pubkey`
- **Replay protection**: timestamp freshness (< 10s old) + monotonic sequence number per session
- **Least privilege**: runs as dedicated system user `nmd`, systemd hardening directives (`NoNewPrivileges`, `ProtectSystem=strict`)

## Build & Install

```bash
# From workspace root:
cargo build --release -p nmd-service

# On remote machine:
./nmd-service/install-scripts/install.sh    # One-command install + start
systemctl status nmd                         # Verify running
journalctl -u nmd -f                        # Follow logs
```

## Current Status

This crate is **Production-Ready** — all modules implemented and tested:
- ✅ metrics collection via metrics-core
- ✅ ChaCha20-Poly1305 AEAD encryption with ECDH pairing
- ✅ UDP transmission with replay protection
- ✅ systemd service with security hardening
- ✅ Automated installation scripts