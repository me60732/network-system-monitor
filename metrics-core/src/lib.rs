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
pub use disk::{DiskStats, DiskIoStats, PartitionStat};
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
/// Since this function cannot maintain state between calls, it creates fresh collectors, primes
/// them with one read, and immediately collects to get accurate deltas from that initial baseline.
///
/// Memory, disk, network, and uptime metrics all read from `/proc` via sysinfo.
/// This function creates a single `sysinfo::System` instance with `new_all()` to minimize
/// syscall overhead — one batched read covers memory and processes.
///
/// ## CPU Measurement Accuracy Note
///
/// This function uses [`CpuCollector::new()`] + `collect()` for accurate delta measurement.
/// The collector is initialized fresh each call, so the first read primes state and the second
/// (in collect()) computes deltas from that baseline. This yields more accurate CPU percentages
/// than single-snapshot approaches but adds ~1-2ms overhead.
pub fn collect_all() -> (CpuStats, MemoryStats, DiskStats, NetworkStats, UptimeStats, GpuStats) {
    // Create a System instance and refresh all data sources in one batched pass.
    // new_all() refreshes CPU, memory, and processes from /proc/stat and /proc/meminfo.
    let sys = sysinfo::System::new_all();
    
    // Use stateful CPU collector for accurate delta measurement (includes CPU temp)
    let mut cpu_collector = CpuCollector::new();  // Prime with initial read + discover CPU temp sensor
    let cpu_stats = cpu_collector.collect();      // Compute deltas from baseline + read CPU temp

    // Build results from the shared System instance — no additional syscalls per module.
    let memory_stats = MemoryStats {
        total: sys.total_memory(),
        free: sys.free_memory(),
        // used = total - available (Linux semantics, matches standard accounting)
        available: sys.available_memory(),
        used: sys.total_memory().saturating_sub(sys.available_memory()),
        swap_used: if sys.total_swap() > 0 {
            ((sys.used_swap() as f64 / sys.total_swap() as f64) * 100.0) as f32
        } else {
            0.0
        },
    };

    let disk_stats = disk::collect();
    let networks = sysinfo::Networks::new_with_refreshed_list();
    // Note: sysinfo 0.39 Networks data does not expose packet counters or dropped counts.
    // Those require direct /proc/net/dev parsing (third field: packets, fourth: dropped).
    // For now, we report None; future enhancement could add procfs-based packet stats.
    let network_stats = NetworkStats {
        interfaces: networks
            .list()
            .iter()
            .map(|(name, data)| InterfaceStat {
                name: name.clone(),
                rx_bytes: data.received(),
                tx_bytes: data.transmitted(),
                rx_packets: None,
                tx_packets: None,
                rx_dropped: None,
                tx_dropped: None,
            })
            .collect(),
    };

    let uptime_stats = UptimeStats {
        seconds: sysinfo::System::uptime(),
        load_avg: parse_loadavg_for_lib().unwrap_or((0.0, 0.0, 0.0)),
    };

    // GPU uses direct sysfs reads or NVML — cannot be batched through sysinfo (includes GPU temp)
    let gpu_stats = gpu::collect();

    (cpu_stats, memory_stats, disk_stats, network_stats, uptime_stats, gpu_stats)
}

/// Parse `/proc/loadavg` and return the three load average values as a tuple of f32.
fn parse_loadavg_for_lib() -> Option<(f32, f32, f32)> {
    use std::fs;

    let content = fs::read_to_string("/proc/loadavg").ok()?;
    let parts: Vec<&str> = content.split_whitespace().collect();

    if parts.len() < 3 {
        return None;
    }

    Some((
        parts[0].parse::<f64>().ok()? as f32,
        parts[1].parse::<f64>().ok()? as f32,
        parts[2].parse::<f64>().ok()? as f32,
    ))
}
