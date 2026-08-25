# metrics-core

Shared Rust library for collecting Linux system metrics. Used by both `nmd-service` (remote systemd service) and `cosmic-applet` (desktop panel widget) in the Network System Monitor project.

## Collected Metrics

| Module | Struct | Key Fields |
|--------|--------|------------|
| `cpu` | `CpuStats` | Aggregate usage %, per-core breakdown |
| `memory` | `MemoryStats` | Total/used/free RAM (bytes), swap % |
| `disk` | `DiskStats` / `PartitionStat` | Per-mount partition total/used bytes |
| `network` | `NetworkStats` / `InterfaceStat` | RX/TX byte counters per interface |
| `uptime` | `UptimeStats` | Seconds since boot, 1/5/15-min load averages |
| `gpu` | `GpuStats` | VRAM total/used (bytes) — `None` if no GPU |
| `temperature` | `TemperatureStats` | CPU/GPU temp (°C) — `None` if unavailable |

## Usage

Add as a dependency in your crate:

```toml
[dependencies]
metrics-core = { path = "../metrics-core" }
```

Collect individual metrics:

```rust
use metrics_core::{cpu, memory};

let cpu_stats = cpu::collect();
println!("CPU usage: {:.1}%", cpu_stats.usage);

let mem_stats = memory::collect();
println!("Memory used: {} bytes / {} bytes", mem_stats.used, mem_stats.total);
```

Collect all metrics at once (primary entry point for `nmd-service`):

```rust
use metrics_core;

let (_cpu, _mem, _disk, _net, _uptime, _gpu, _temp) = metrics_core::collect_all();
```

## Performance Target

Full collection of all modules must complete in **< 50ms** for real-time panel updates. See `benches/full_suite.rs` and the [ImplementationGuide](https://github.com/mark/network-system-monitor/blob/main/.planning_docs/ImplementationGuide.md) for benchmark details.

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `rkyv-serializable` | No | Enables `rkyv::Archive` derives on all structs (used by `nmd-service`) |

Enable via: `metrics-core = { version = "...", features = ["rkyv-serializable"] }`

## Development Phases

This crate follows the TNG agent workflow handoff model:
- **Phase 1A** — Geordi scaffolds stubs + doc drafts (this phase) ✅
- **Phase 1B** — Beverly implements real collection logic via sysinfo/procfs, writes unit tests
- **Phase 1C** — Worf audits procfs/sysinfo access for security issues
- **Phase 1D** — Troi completes all module/API documentation

See `.planning_docs/` for full architecture and implementation details.
