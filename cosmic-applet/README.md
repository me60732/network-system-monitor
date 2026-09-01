# cosmic-applet — Network System Monitor Desktop Panel Applet

A Cosmic desktop panel applet that displays system metrics from all Linux machines on your local network in a single unified view. No SSH or manual login required — each remote machine runs `nmd` (systemd service) that pushes rkyv-encoded, ChaCha20-Poly1305 AEAD encrypted UDP packets to the desktop applet every 1 second.

## Features

- **Panel Widget**: Single-line Cosmic panel showing local CPU, memory, disk, network, uptime, GPU VRAM, and temperature stats with color thresholds at 60% (yellow) / 80% (red).
- **Machine List UI**: Always-visible list of all remote machines — one row per machine with status indicators (● online / ○ offline/pending) and per-machine sensor configuration.
- **Per-Machine Sensor Config**: Each machine has its own configurable sensor display via gear icon in machine detail view. Control what shows in the panel row for each machine independently.
- **Global Settings**: Apply to ALL machines: value size, monospace font, panel spacing, content order.
- **Secure UDP Protocol**: ChaCha20-Poly1305 AEAD encryption + replay protection via timestamp freshness (< 10s) and per-machine sequence number tracking.
- **TOML Configuration**: Machine registration with per-machine metric selection, refresh rate configured per-machine via nmd-service config file.

## Project Structure (pop-os/cosmic-applet-template layout)

```
cosmic-applet/
├── Cargo.toml          # Dependencies: cosmic, iced, metrics-core, rkyv, chacha20, poly1305, toml, serde, tokio
├── justfile            # Build recipes: build-release, install, vendor, check (clippy), check-json
├── README.md           # This file
├── src/
│   ├── main.rs              # Applet entry point — PanelWidget registration + machine list UI
│   ├── panel_widget.rs      # Single-line Cosmic panel rendering with 60%/80% color thresholds (<1s load)
│   ├── machine_list.rs      # Always-visible list of all remote machines (no dual panel/list mode)
│   ├── machine_detail.rs    # Per-machine detail view: panel row at top, all metrics NOT in row below
│   ├── machine_sensor_config_menu.rs  # Per-machine sensor row config (opens via gear icon in machine detail)
│   ├── sensor_config.rs     # Individual sensor config panels (CPU, memory, network, disk, GPU)
│   ├── settings_window.rs   # General/global settings: value size, monospace font, panel spacing, content order
│   ├── config_manager.rs    # TOML config loading/saving, manages machine list + per-machine sensor configs
│   ├── udp_receiver.rs      # Listens for rkyv-encoded MetricPacket via UDP + ChaCha20-Poly1305 AEAD verification
│   └── pairing_manager.rs   # TOFU pairing UI with accept/deny dropdown, ECDH key derivation
├── benches/
│   ├── panel_bench.rs      # Benchmark: PanelWidget render time (<10ms target)
│   └── machine_list_bench.rs  # Benchmark: Machine list update with N machines (<50ms target)
├── data/
│   ├── appdata.xml.in      # AppStream metadata template (Cosmic standards)
│   └── icons/              # Applet icon placeholders — 16x16 through 512x512 + symbolic variants
└── docs/
    ├── user-guide.md        # User-facing documentation
    └── applet-config.md     # TOML config reference with all options explained
```

## Development

### Prerequisites

- Rust toolchain (rustc 1.74+) — matches workspace `rust-version`
- Cosmic desktop environment development headers (`cosmic` crate v0.6)

### Build & Install

```bash
# Build in release mode:
just build-release

# Install to ~/.local/bin/:
just install

# Run clippy checks (cosmic-utils standard):
just check

# Vendor dependencies for offline builds:
just vendor
```

## Architecture Integration Points

This crate integrates with the rest of the workspace as follows:

| Component       | Source Crate    | Consumed By          | Purpose                          |
|-----------------|-----------------|----------------------|----------------------------------|
| `MetricPacket`  | `nmd-service/src/packet.rs` | `udp_receiver.rs` | rkyv-serialized UDP payload with ChaCha20-Poly1305 AEAD + replay protection |
| Metric structs  | `metrics-core/src/*.rs`     | `panel_widget.rs`, `machine_detail.rs` | CPU, memory, disk, network, uptime, GPU, temperature stats for local and remote display |
| `ServiceConfig` | `nmd-service/src/config.rs` | (reference only)    | Shared config format conventions for machine_id + host/port |
| `MachineConfig` | `cosmic-applet/src/config_manager.rs` | UI components      | Per-machine sensor configuration, refresh rate, pairing data |
| `UserPreferences` | `cosmic-applet/src/settings_window.rs` | UI components   | Global settings: value size, monospace font, panel spacing, content order |

## Security (Worf Phase 1A)

- **Encryption**: ChaCha20-Poly1305 AEAD (confidentiality + authenticity in one operation)
- **Pairing**: Trust-On-First-Use (TOFU) — receiver detects unknown senders, shows pairing UI with accept/deny dropdown
- **Replay Protection**: Timestamp freshness (< 10s old) + monotonic sequence number tracking per `machine_id` session
- **Per-Machine Keys**: ECDH-derived shared keys stored in `~/.config/cosmic-applet/pairing.toml` after acceptance

## License

MIT OR Apache-2.0