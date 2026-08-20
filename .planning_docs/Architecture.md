---
tags: [pipeline, architecture, network-system-monitor]
created: 2025-07-13
---

# Architecture: Network System Monitor

## Tech Stack

| Layer | Choice | Reason |
|---|---|---|
| Language | Rust | User's preference; performance for system metrics collection |
| GUI Framework | cosmic-lib / iced | Same framework as Cosmic desktop — keeps it consistent with the desktop itself |
| Data Transport | UDP push with rkyv serialization (single-direction, no retries) | Low overhead one-way transport; remote machines send metrics to desktop on schedule. No auto-discovery needed — each machine pre-configured with host IP. Machine ID included in packet for auto-registration. ✅ Confirmed by Captain 
| Metrics Collection | procfs + sysinfo crate | Standard Linux system stats: CPU, memory, disk, network |
| Config Format | TOML | Consistent with Rust ecosystem; minimon-applet likely uses similar format |
| Remote Service | systemd unit | User's preference — each machine runs a service sending data to desktop |

## Key Components

```
Remote Machine (each)
  └── systemd service (metrics collector + sender)
        ↓ Binary (rkyv/Cap'n Proto)
Desktop Machine
  ├── cosmic applet (panel widget)
  │     └── click → expand window showing all remote machines
  ├── config manager (TOML-based machine/metric selection)
  └── local system monitor (desktop's own stats, opened from applet)
```

## External Dependencies

| Dependency | Purpose | Risk if unavailable |
|---|---|---|
| cosmic-lib / iced | GUI framework for Cosmic applet | Core dependency — must be available and stable |
| sysinfo crate | System metrics collection (CPU, RAM, disk, network) | Standard Rust crate; low risk |
| procfs | Linux kernel interface for detailed stats | Linux-only; works on all target machines |
| minimon-applet source | UI/design reference patterns | Optional — can design from scratch if needed |

## Key Technical Decisions

### Decision 1: Data Transport Protocol — Resolved ✅ (UDP Push)
**Choice**: UDP push with rkyv serialization, single-direction no retries. Each remote machine pre-configured with desktop host IP. Machine ID included in packet for auto-registration when new services come online.

### Decision 2: systemd Service Design
**Choice**: Separate binary crate vs part of applet codebase  
**Reason**: The remote service is fundamentally different from the desktop applet — it's a daemon that collects and sends metrics, not a GUI. A separate binary keeps concerns clean. Can share a common `metrics-core` crate for data collection between both.  
**Trade-off**: Separate = more binaries to maintain; shared = code reuse but tighter coupling

### Decision 3: Authentication Between Machines — Updated ⚠️ (Re-evaluate)
**Current**: mTLS/certificate auth was confirmed in lavish review. **But with UDP push model, reconsider:** certificates over UDP is unusual. May need to pivot to pre-shared token or HMAC for MVP. Self-signed certs still viable if we switch to TCP for authenticated channels later.

## Open Technical Questions
Things that need a spike or research before starting.

- [x] minimon-applet source code reviewed for UI patterns and config format → Phase 0 reference
- [x] cosmic-lib applet API confirmed well-documented (v1.3-v1.4) → Panel widget creation pattern established
- [ ] systemd service: push vs pull? (push = simpler desktop, pull = simpler remote)
- [x] Network discovery: auto-discovery approved ✅

## Related ADRs
Link any ADRs that apply to this project's decisions.

## Cosmic Utils Standards
- Use `pop-os/cosmic-applet-template` as official template (not cosmic-utils own)
- justfile recipes required: build-release, install, vendor, check (clippy), check-json
- Flatpak via cosmic.flatpakrepo source for app store visibility
- Installation paths follow standard Linux conventions (/usr/bin, /usr/share/appdata/, etc.)
- Open PR against `cosmic-utils/cosmic-project-collection` with entry in appropriate RON file