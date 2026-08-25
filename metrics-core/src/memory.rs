//! Memory and swap statistics.
//!
//! Collects RAM and swap utilization using the `sysinfo` crate, which reads from
//! `/proc/meminfo` on Linux. All values are in bytes unless noted otherwise.
//!
//! ## Data Flow
//!
//! 1. `collect()` creates a fresh [`sysinfo::System`] with all features enabled (`new_all`).
//! 2. It refreshes memory and swap data via `refresh_memory()`, which reads `/proc/meminfo`.
//! 3. Total/used/free RAM come from `SystemExt` methods: `total_memory()`, `used_memory()`, `free_memory()`.
//! 4. Swap usage percentage is computed as `(swap_used / swap_total) * 100.0`, or 0.0 if no swap exists.

use serde::{Deserialize, Serialize};
use sysinfo::System;

/// Memory and swap utilization statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total physical RAM in bytes.
    pub total: u64,
    /// Currently used RAM in bytes (total - available).
    pub used: u64,
    /// Completely free/unused RAM in bytes.
    pub free: u64,
    /// Swap usage as a percentage of total swap space (0.0–100.0).
    pub swap_used: f32,
}

/// Collect current memory statistics.
///
/// Uses `sysinfo::System` to read `/proc/meminfo` and report RAM and swap utilization.
/// All byte values are in bytes. `swap_used` is a percentage (0.0–100.0) representing
/// how much of total swap space is currently in use; returns 0.0 if no swap partition exists.
pub fn collect() -> MemoryStats {
    let sys = System::new_all();

    let total = sys.total_memory();
    let free = sys.free_memory();
    // used = total - available (sysinfo's used_memory() already computes this)
    let used = sys.used_memory();

    let swap_total = sys.total_swap();
    let swap_used_bytes = sys.used_swap();
    let swap_used_percent = if swap_total > 0 {
        ((swap_used_bytes as f64 / swap_total as f64) * 100.0) as f32
    } else {
        0.0
    };

    MemoryStats {
        total,
        used,
        free,
        swap_used: swap_used_percent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// used ≤ total, free ≥ 0 (Beverly writes after implementation).
    #[test]
    fn test_memory_used_le_total() {
        let stats = collect();
        assert!(stats.used <= stats.total);
        // free is u64, so always >= 0 — removed useless comparison warning.
    }

    /// Swap usage percentage must be within bounds when swap exists.
    #[test]
    fn test_swap_usage_within_bounds() {
        let stats = collect();
        // If there's no swap, percentage is 0.0; otherwise it should be 0–100%.
        assert!(stats.swap_used >= 0.0 && stats.swap_used <= 100.0);
    }

    /// Total memory must be positive on any running Linux system.
    #[test]
    fn test_total_memory_positive() {
        let stats = collect();
        assert!(stats.total > 0);
    }
}
