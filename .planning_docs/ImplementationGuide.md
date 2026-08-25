# Implementation Guide: Network System Monitor

tags: [pipeline, implementation, network-system-monitor]
status: 🚧 Phase 1 Planning — Detailed Module Breakdown & Test Plans
created: 2026-08-20

## Purpose

This document is the definitive development reference for all agents implementing the Network System Monitor. It maps every module across three crates (`metrics-core`, `nmd-service`, `cosmic-applet`), specifies required stubs, unit tests, benchmarks, and documentation deliverables. The Lavish design artifact (`.lavish/network-monitor-design.html`) was used during planning only — this guide is what developers follow during implementation.

---

## Crate 1: metrics-core (Shared Library)

### Module Map & Stubs

| Module | File Path | Key Functions/Types | Stub Requirements |
|--------|-----------|---------------------|-------------------|
| `lib.rs` | `src/lib.rs` | Re-exports all submodules, crate-level docs | Empty function bodies with `// TODO` comments |
| `cpu.rs` | `src/cpu.rs` | `CpuStats { usage: f32, cores: Vec<CoreStat> }`, `fn collect() -> CpuStats` | Return zeroed structs |
| `memory.rs` | `src/memory.rs` | `MemoryStats { total: u64, used: u64, free: u64, swap_used: f32 }`, `fn collect() -> MemoryStats` | Return zeroed structs |
| `disk.rs` | `src/disk.rs` | `DiskStats { partitions: Vec<PartitionStat> }`, `PartitionStat { mount: String, total: u64, used: u64 }` | Return empty vec |
| `network.rs` | `src/network.rs` | `NetworkStats { interfaces: Vec<InterfaceStat> }`, `InterfaceStat { name: String, rx_bytes: u64, tx_bytes: u64 }` | Return empty vec |
| `uptime.rs` | `src/uptime.rs` | `UptimeStats { seconds: u64, load_avg: (f32, f32, f32) }`, `fn collect() -> UptimeStats` | Return zeroed struct |
| `gpu.rs` | `src/gpu.rs` | `GpuStats { vram_total: Option<u64>, vram_used: Option<u64> }`, `fn collect() -> GpuStats` | Return None for VRAM (unsupported on some systems) |
| `temperature.rs` | `src/temperature.rs` | `TemperatureStats { cpu_temp: Option<f32>, gpu_temp: Option<f32> }`, `fn collect() -> TemperatureStats` | Return None values |

### Unit Tests Required

| Module | Test Name | Description | Agent Responsible |
|--------|-----------|-------------|-------------------|
| `cpu.rs` | `test_cpu_usage_within_bounds` | CPU usage percentage must be 0.0–100.0 | Beverly (Geordi writes) |
| `cpu.rs` | `test_core_count_matches_sysinfo` | Number of cores matches sysinfo output | Beverly (Geordi writes) |
| `memory.rs` | `test_memory_used_le_total` | used ≤ total, free ≥ 0 | Beverly (Geordi writes) |
| `disk.rs` | `test_disk_partitions_nonempty` | At least one partition returned on Linux | Beverly (Geordi writes) |
| `network.rs` | `test_network_interfaces_present` | Loopback or eth0/wlan0 present | Beverly (Geordi writes) |
| `uptime.rs` | `test_uptime_positive` | Uptime seconds > 0 after boot | Beverly (Geordi writes) |
| `temperature.rs` | `test_temp_optional_handling` | Handles None gracefully on unsupported hardware | Beverly (Geordi writes) |

### Benchmarks Required

| Benchmark | Target File | Description | Agent Responsible |
|-----------|-------------|-------------|-------------------|
| `bench_cpu_collect` | `benches/cpu_bench.rs` | Measure time to collect full CPU stats | Geordi (Beverly validates) |
| `bench_memory_collect` | `benches/memory_bench.rs` | Measure memory collection overhead | Geordi (Beverly validates) |
| `bench_all_metrics` | `benches/full_suite.rs` | End-to-end metrics collection time | Geordi (Beverly validates) |

**Performance target**: Full metrics collection must complete in < 50ms for real-time panel updates.

### Documentation Required

| Doc Type | File Path | Content | Agent Responsible |
|----------|-----------|---------|-------------------|
| Module docs | `src/lib.rs` | Crate-level overview, usage examples | Troi (Geordi writes draft) |
| API reference | Each `.rs` file | Per-module doc comments on all public functions/types | Troi (Geordi writes draft) |
| README | `README.md` | Quick start: how to use metrics-core as a dependency | Troi |

---

## Crate 2: nmd-service (Remote Systemd Service)

### Module Map & Stubs

| Module | File Path | Key Functions/Types | Stub Requirements |
|--------|-----------|---------------------|-------------------|
| `main.rs` | `src/main.rs` | `fn main()` — systemd entry point, signal handling | Parse args, exit cleanly |
| `config.rs` | `src/config.rs` | `ServiceConfig { host: String, port: u16, interval_ms: u64 }`, `fn load() -> ServiceConfig` | Hardcoded defaults |
| `udp_sender.rs` | `src/udp_sender.rs` | `UdpSender { socket: UdpSocket, dest: SocketAddr }`, `fn send(&self, packet: &MetricPacket)` | No-op or log-only |
| `packet.rs` | `src/packet.rs` | `MetricPacket { machine_id: String, timestamp: u64, metrics: MetricsData }` (rkyv-serializable) | Empty struct with derive macros |
| `metrics_aggregator.rs` | `src/metrics_aggregator.rs` | `fn aggregate() -> MetricPacket` — calls all metrics-core collectors and packs into rkyv format | Return empty packet |
| `systemd_unit.rs` | `src/systemd_unit.rs` | Constants for systemd unit file content, install/uninstall helpers | String constants only |

### Unit Tests Required

| Module | Test Name | Description | Agent Responsible |
|--------|-----------|-------------|-------------------|
| `config.rs` | `test_config_defaults` | Default config has valid host/port/interval | Beverly (Geordi writes) |
| `packet.rs` | `test_packet_rkyv_roundtrip` | Serialize + deserialize MetricPacket via rkyv preserves data | Beverly (Geordi writes) |
| `udp_sender.rs` | `test_send_to_invalid_addr_fails_gracefully` | Sending to unreachable address doesn't panic | Beverly (Geordi writes) |
| `metrics_aggregator.rs` | `test_aggregate_returns_machine_id` | Aggregated packet contains correct machine ID | Beverly (Geordi writes) |

### Benchmarks Required

| Benchmark | Target File | Description | Agent Responsible |
|-----------|-------------|-------------|-------------------|
| `bench_packet_serialization` | `benches/packet_bench.rs` | Measure rkyv serialization time for MetricPacket | Geordi (Beverly validates) |
| `bench_aggregate_overhead` | `benches/aggregator_bench.rs` | Time to aggregate all metrics into packet | Geordi (Beverly validates) |

**Performance target**: Full aggregation + rkyv serialization must complete in < 5ms. UDP send overhead negligible (< 1ms).

### Documentation Required

| Doc Type | File Path | Content | Agent Responsible |
|----------|-----------|---------|-------------------|
| Module docs | `src/main.rs` | Service lifecycle, systemd integration notes | Troi (Geordi writes draft) |
| Config reference | `docs/config.md` | All configuration options with examples | Troi |
| Install guide | `install-scripts/README.md` | How to install on remote machines, cert generation | Troi |

---

## Crate 3: cosmic-applet (Desktop Applet)

### Module Map & Stubs

| Module | File Path | Key Functions/Types | Stub Requirements |
|--------|-----------|---------------------|-------------------|
| `main.rs` | `src/main.rs` | Cosmic applet entry point, panel widget registration | Print "Hello World" in panel |
| `panel_widget.rs` | `src/panel_widget.rs` | `PanelWidget { stats: DesktopStats }`, renders CPU/mem/disk/network/uptime/GPU/VRAM/temp in single-line format | Static placeholder values |
| `grid_window.rs` | `src/grid_window.rs` | `GridWindow { machines: Vec<MachineRow> }`, click-to-expand grid showing all remote machines | Empty grid with headers only |
| `machine_row.rs` | `src/machine_row.rs` | `MachineRow { name: String, status: Status, metrics: MetricsData }` — one row per machine in grid | Hardcoded "Pending" rows |
| `config_manager.rs` | `src/config_manager.rs` | `ConfigManager { machines: Vec<MachineConfig> }`, loads/saves TOML config | Load hardcoded defaults |
| `udp_receiver.rs` | `src/udp_receiver.rs` | `UdpReceiver { socket: UdpSocket }`, listens for incoming UDP packets, updates grid in real-time | Log received packets only |
| `status_indicator.rs` | `src/status_indicator.rs` | `StatusIndicator` — renders ● (online), ○ (offline/pending) with color thresholds at 60%/80% | Static indicators only |
| `local_monitor.rs` | `src/local_monitor.rs` | Opens desktop's own system monitor from applet click | Print "Open local monitor" to console |

### Unit Tests Required

| Module | Test Name | Description | Agent Responsible |
|--------|-----------|-------------|-------------------|
| `config_manager.rs` | `test_load_config_defaults` | Loading default config returns expected machine list | Beverly (Geordi writes) |
| `config_manager.rs` | `test_add_remove_machine` | Adding/removing machines persists to TOML correctly | Beverly (Geordi writes) |
| `udp_receiver.rs` | `test_parse_incoming_packet` | Correctly parses MetricPacket from UDP bytes via rkyv | Beverly (Geordi writes) |
| `panel_widget.rs` | `test_panel_load_time` | Panel widget loads in < 1s on applet startup | Beverly (Geordi writes) |
| `status_indicator.rs` | `test_threshold_colors` | Color thresholds at 60%/80% applied correctly for each metric type | Beverly (Geordi writes) |

### Benchmarks Required

| Benchmark | Target File | Description | Agent Responsible |
|-----------|-------------|-------------|-------------------|
| `bench_panel_render` | `benches/panel_bench.rs` | Measure panel widget render time from metrics data | Geordi (Beverly validates) |
| `bench_grid_update` | `benches/grid_bench.rs` | Time to update grid window with N machine rows | Geordi (Bordeus validates) |

**Performance target**: Panel renders in < 10ms. Grid updates with full remote machine list must complete in < 50ms.

### Documentation Required

| Doc Type | File Path | Content | Agent Responsible |
|----------|-----------|---------|-------------------|
| Module docs | `src/main.rs` | Applet architecture, Cosmic integration notes | Troi (Geordi writes draft) |
| User guide | `docs/user-guide.md` | How to configure machines, use the panel widget and grid window | Troi |
| Config reference | `docs/applet-config.md` | TOML config schema with all options explained | Troi |

---

## Cross-Crate Integration Points

### 1. rkyv Packet Format (`nmd-protocol`)

**Note**: This is handled inline within each crate using `rkyv::Archive`, not a separate crate, to reduce complexity for MVP.

| Type | Defined In | Consumed By | Fields |
|------|------------|-------------|--------|
| `MetricPacket` | `nmd-service/src/packet.rs` | `cosmic-applet/src/udp_receiver.rs` | machine_id, timestamp, cpu_usage, memory_used_percent, disk_used_percent, network_rx_bytes, uptime_seconds, gpu_vram_used_mb (Option), temperature_celsius (Option) |

### Directory Structure Overview

The project follows the `pop-os/cosmic-applet-template` layout from day one. All three crates live in a single workspace for shared dependency management:

```
network-system-monitor/
├── .planning_docs/                    # Design docs and planning materials (vault-linked, not published)
│   ├── Brief.md
│   ├── Architecture.md
│   ├── Goals.md
│   ├── Scope.md
│   ├── Risks.md
│   ├── Roadmap.md
│   ├── ImplementationGuide.md        ← This file — agent development reference
│   └── Index.md
├── .lavish/                          # Interactive design review artifacts (planning only)
│   └── network-monitor-design.html
├── metrics-core/                     # Shared library crate: system metrics collection
│   ├── Cargo.toml
│   ├── README.md
│   ├── src/
│   │   ├── lib.rs                    ← Crate root, re-exports all modules
│   │   ├── cpu.rs                    ← CPU usage stats (per-core + aggregate)
│   │   ├── memory.rs                 ← Memory/swap stats via sysinfo
│   │   ├── disk.rs                   ← Disk partition stats via procfs/sysinfo
│   │   ├── network.rs                ← Network interface RX/TX bytes
│   │   ├── uptime.rs                 ← System uptime + load averages
│   │   ├── gpu.rs                    ← GPU VRAM (optional — None on unsupported)
│   │   └── temperature.rs            ← CPU/GPU temps (optional — None if unavailable)
│   └── benches/                      # Performance benchmarks
│       ├── cpu_bench.rs              ← bench_cpu_collect (< 50ms target)
│       ├── memory_bench.rs           ← bench_memory_collect
│       └── full_suite.rs             ← bench_all_metrics (end-to-end < 50ms)
├── nmd-service/                      # Remote systemd service crate (runs on Pluto, Spark, etc.)
│   ├── Cargo.toml
│   ├── README.md
│   ├── src/
│   │   ├── main.rs                   ← Entry point: systemd lifecycle + signal handling
│   │   ├── config.rs                 ← ServiceConfig (host, port, interval) — TOML-based
│   │   ├── udp_sender.rs             ← UdpSender: sends rkyv-encoded MetricPacket to desktop
│   │   ├── packet.rs                 ← MetricPacket struct + rkyv::Archive derive macros
│   │   ├── metrics_aggregator.rs     ← Calls metrics-core, packs into rkyv format
│   │   └── systemd_unit.rs           ← Systemd unit file constants + install/uninstall helpers
│   ├── benches/                      # Performance benchmarks
│   │   ├── packet_bench.rs           ← bench_packet_serialization (< 5ms target)
│   │   └── aggregator_bench.rs       ← bench_aggregate_overhead
│   └── install-scripts/              ← Remote machine deployment scripts (Phase 1D)
│       ├── install.sh                ← Auto-generates config, installs systemd unit + binary
│       ├── generate-certs.sh         ← Self-signed cert generation (mTLS or token-based TBD)
│       └── README.md                 ← Install guide documentation
├── cosmic-applet/                    # Desktop Cosmic applet crate (runs on desktop machine)
│   ├── Cargo.toml                    ← Based on pop-os/cosmic-applet-template
│   ├── justfile                      ← build-release, install, vendor, check, check-json recipes
│   ├── README.md
│   ├── src/
│   │   ├── main.rs                   ← Applet entry point: panel widget registration
│   │   ├── panel_widget.rs           ← Single-line desktop stats in Cosmic panel (< 1s load)
│   │   ├── grid_window.rs            ← Click-to-expand window showing all remote machines
│   │   ├── machine_row.rs            ← One row per machine: name, status ●/○, metrics grid
│   │   ├── config_manager.rs         ← Loads/saves TOML config (extends minimon-applet format)
│   │   ├── udp_receiver.rs           ← Listens for UDP packets from remote machines via rkyv
│   │   ├── status_indicator.rs       ← Renders ●/○ with 60%/80% color thresholds
│   │   └── local_monitor.rs          ← Opens desktop's own system monitor (click handler)
│   ├── benches/                      # Performance benchmarks
│   │   ├── panel_bench.rs            ← bench_panel_render (< 10ms target)
│   │   └── grid_bench.rs             ← bench_grid_update with N machine rows (< 50ms)
│   ├── docs/
│   │   ├── user-guide.md             ← How to configure machines, use panel + grid window
│   │   └── applet-config.md          ← TOML config schema reference
│   └── data/                         # Cosmic applet template standard directories
│       ├── icons/                    ← Applet icons (panel widget)
│       ├── screenshots/              ← Screenshots for cosmic-utils.org listing (Phase 4)
│       └── appdata.xml.in            ← AppData XML for package metadata
├── Cargo.toml                        # Workspace manifest: members = [metrics-core, nmd-service, cosmic-applet]
├── .gitignore                        # Ignores /target/, IDE files, OS artifacts
└── README.md                         # Project overview — links to all planning docs

### 2. Shared Metrics Data (`metrics-core`)

| Type | Defined In | Consumed By | Usage |
|------|------------|-------------|-------|
| `CpuStats` | `metrics-core/src/cpu.rs` | Both crates collect and display | CPU usage percentage + per-core breakdown |
| `MemoryStats` | `metrics-core/src/memory.rs` | Both crates | Total/used/free memory in bytes |

### 3. Config Schema (`nmd-config`)

**Note**: Extends minimon-applet's TOML format, not a separate crate for MVP.

| Section | Fields | Used By |
|---------|--------|---------|
| `[machines]` | Array of machine configs (name, host_ip, port) | `nmd-service` (sender), `cosmic-applet` (receiver/grid display) |
| `[metrics]` | Enabled metrics per machine (cpu, memory, disk, network, uptime, gpu_vram, temperature) | Both crates determine what to collect/display |

---

## Agent Workflow Summary Table

| Deliverable | Geordi Tasks | Beverly Tasks | Worf Tasks | Troi Tasks |
|-------------|--------------|---------------|------------|------------|
| **metrics-core** | Create 8 modules with stubs + docs drafts; write benchmarks | Write/run 7 unit tests, validate 3 benchmarks meet targets | Audit procfs/sysinfo access for security issues | Complete all module/API documentation |
| **nmd-service** | Create 6 modules with stubs + config/install docs draft; write benchmarks | Write/run 4 unit tests, validate 2 benchmarks | Review UDP auth strategy, check privilege escalation risks | Complete config reference and install guide |
| **cosmic-applet** | Scaffold from template, create 8 modules with stubs + usage doc draft; write benchmarks | Write/run 5 unit tests, validate 2 benchmarks | Audit applet config handling for injection/tampering | Complete user guide and config schema docs |

---

## Key Decisions Impacting Implementation

1. **UDP Push Model**: No retries needed — just resend next cycle. Each remote machine pre-configured with desktop host IP in `config.toml`.
2. **rkyv Serialization Only (MVP)**: Cap'n Proto deferred to Phase 2 if cross-platform support becomes necessary.
3. **HMAC-SHA256 Authentication (Confirmed by Worf)**: Pivot from mTLS/DTLS to HMAC-signed packets with pre-shared key. MetricPacket augmented with `timestamp`, `sequence`, and `hmac_tag` fields. Secret distributed via `/etc/nmd/secret.key`. Replay protection via timestamp freshness + sequence number tracking.
4. **No Auto-Discovery**: Remote machines auto-register on first UDP packet arrival (machine ID included in packet).
5. **Template-First Repo Structure**: Repository structured following `pop-os/cosmic-applet-template` conventions from day one — no Phase 4 refactoring needed.