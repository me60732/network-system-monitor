//! CPU usage statistics.
//!
//! Collects aggregate and per-core CPU utilization using the `sysinfo` crate, which reads
//! from `/proc/stat` on Linux. Usage is expressed as a percentage (0.0–100.0).
//!
//! ## Data Flow
//!
//! 1. `collect()` creates a fresh [`sysinfo::System`] instance with CPU support enabled.
//! 2. It refreshes the full system state in a single batched syscall pass (`refresh_all`).
//! 3. Aggregate usage is read from `System::cpu_usage()`, which returns 0.0–100.0.
//! 4. Per-core data comes from iterating `System::cpus()` — each entry provides
//!    `Cpu::usage()` (percentage) and `Cpu::frequency()` (MHz).
//! 5. The result is consumed by `nmd-service` for rkyv serialization into `MetricPacket`.

use serde::{Deserialize, Serialize};
use sysinfo::System;

/// Per-core CPU usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoreStat {
    /// Zero-indexed core number (0 = first logical processor).
    pub index: u32,
    /// Current usage percentage for this core (0.0–100.0).
    pub usage: f32,
}

/// Aggregate CPU statistics across all cores.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuStats {
    /// Overall system-wide CPU usage percentage (0.0–100.0).
    pub usage: f32,
    /// Per-core breakdown — one entry per logical processor.
    pub cores: Vec<CoreStat>,
}

/// Collect current CPU statistics.
///
/// Uses `sysinfo::System` to read `/proc/stat` and compute CPU utilization percentages.
/// The aggregate `usage` field reflects overall system-wide CPU usage (0.0–100.0).
/// Each entry in `cores` corresponds to one logical processor with its current usage percentage.
///
/// **Performance target**: < 50ms for full metrics suite including this call.
/// This function performs a single batched refresh, minimizing syscall overhead.
pub fn collect() -> CpuStats {
    let sys = System::new_all();

    // global_cpu_usage() returns aggregate CPU percentage (0.0–100.0).
    // new_all() already refreshed CPU data from /proc/stat in a single batched pass.
    let usage = sys.global_cpu_usage();
    let cores: Vec<CoreStat> = sys
        .cpus()
        .iter()
        .enumerate()
        .map(|(i, cpu)| CoreStat {
            index: i as u32,
            usage: cpu.cpu_usage(),
        })
        .collect();

    CpuStats { usage, cores }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysinfo::System;

    /// CPU usage percentage must be 0.0–100.0 (Beverly writes after implementation).
    #[test]
    fn test_cpu_usage_within_bounds() {
        let stats = collect();
        assert!(stats.usage >= 0.0 && stats.usage <= 100.0);
    }

    /// Number of cores must match sysinfo output (Beverly writes after implementation).
    #[test]
    fn test_core_count_matches_sysinfo() {
        let stats = collect();
        // Cross-check against a fresh sysinfo System instance to verify core count matches.
        let sys_check = System::new_all();
        assert_eq!(stats.cores.len(), sys_check.cpus().len());
    }

    /// Per-core usage must also be within bounds (0.0–100.0).
    #[test]
    fn test_core_usage_within_bounds() {
        let stats = collect();
        for core in &stats.cores {
            assert!(core.usage >= 0.0 && core.usage <= 100.0);
        }
    }
}
