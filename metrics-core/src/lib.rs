//! # metrics-core
//!
//! Shared library for collecting system metrics on Linux. This crate provides the core data
//! structures and collection functions used by both `nmd-service` (the remote systemd service)
//! and `cosmic-applet` (the desktop panel widget).
//!
//! ## Modules
//!
//! Each module corresponds to a specific category of system metric:
//! - [`cpu`] — CPU usage percentage and per-core breakdown.
//! - [`memory`] — RAM and swap utilization in bytes.
//! - [`disk`] — Disk partition usage across all mounted filesystems.
//! - [`network`] — Network interface RX/TX byte counters.
//! - [`uptime`] — System uptime in seconds plus 1/5/15-minute load averages.
//! - [`gpu`] — GPU VRAM stats (optional; `None` on unsupported hardware).
//! - [`temperature`] — CPU/GPU temperatures in Celsius (optional; `None` if unavailable).
//!
//! ## Usage as a Dependency
//!
//! Add to your `Cargo.toml`:
//! ```toml
//! [dependencies]
//! metrics-core = { path = "../metrics-core" }
//! ```
//!
//! Then create and use a collector:
//! ```no_run
//! use metrics_core::CpuCollector;
//! let mut collector = CpuCollector::new();
//! let stats = collector.collect();
//! println!("CPU usage: {:.1}%", stats.usage);
//! ```
//!
//! ## Performance Target
//!
//! Full collection (all modules) must complete in < 50ms for real-time panel updates.
//! See `benches/full_suite.rs` and the ImplementationGuide for benchmark details.
//!
//! ## Agent Handoff Notes (Phase 1A → Beverly/Worf/Troi)
//!
//! - **Beverly** writes unit tests in each module's `#[cfg(test)]` block per the
//!   ImplementationGuide test matrix (7 tests across cpu, memory, disk, network, uptime, temperature).
//! - **Worf** audits procfs/sysinfo access for security issues.
//! - **Troi** completes all doc comments — this file has draft-level docs; refine for final API reference.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod uptime;

// Re-export top-level structs for ergonomic imports: `use metrics_core::CpuStats;`
pub use cpu::{CoreStat, CpuStats};
pub use disk::{DiskIoStats, DiskStats, PartitionStat};
pub use gpu::GpuStats;
pub use memory::MemoryStats;
pub use network::{InterfaceStat, NetworkCollector, NetworkStats};
pub use uptime::UptimeStats;

// Re-export stateful collectors
pub use cpu::CpuCollector;
pub use gpu::GpuCollector;

/// Convenience function: collect all metrics in a single pass.
///
/// Returns a tuple of `(CpuStats, MemoryStats, DiskStats, NetworkStats, UptimeStats, GpuStats)`.
/// This is the primary entry point for `nmd-service`'s aggregator and benchmarks.
///
/// **Performance target**: < 50ms total on typical Linux hardware.
///
/// ## Implementation Notes
///
/// CPU uses a stateful collector (`CpuCollector`) for accurate delta measurement and includes
/// CPU temperature in the returned CpuStats. GPU collector includes GPU temperature in GpuStats.
/// Memory, disk, network, and uptime metrics all read from `/proc` via procfs directly.
pub fn collect_all() -> (
    CpuStats,
    MemoryStats,
    DiskStats,
    NetworkStats,
    UptimeStats,
    GpuStats,
) {
    // Use stateful CPU collector for accurate delta measurement (includes CPU temp)
    let mut cpu_collector = CpuCollector::new();
    let cpu_stats = cpu_collector.collect();

    // Direct procfs calls for memory, disk, network, uptime
    let memory_stats = memory::collect();
    let disk_stats = disk::collect();
    let mut network_collector = NetworkCollector::new();
    let network_stats = network_collector.collect();
    let uptime_stats = uptime::collect();

    // GPU uses direct sysfs reads or NVML — cannot be batched through procfs (includes GPU temp)
    let gpu_stats = gpu::collect();

    (
        cpu_stats,
        memory_stats,
        disk_stats,
        network_stats,
        uptime_stats,
        gpu_stats,
    )
}
