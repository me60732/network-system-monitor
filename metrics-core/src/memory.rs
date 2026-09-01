//! Memory and swap statistics.
//!
//! Collects RAM and swap utilization using `procfs`, which reads from
//! `/proc/meminfo` on Linux. All values are in bytes unless noted otherwise.
//!
//! ## Data Flow
//!
//! 1. `collect()` uses `procfs::Meminfo::current()` to read `/proc/meminfo`.
//! 2. procfs provides values in kilobytes — multiply by 1024 to get bytes.
//! 3. Total/used/free RAM are computed from procfs fields:
//!    - **Used memory** = total - available (Linux semantics, matches standard accounting)
//!    - If `mem_available` is missing, fall back to `mem_free`
//! 4. Swap usage percentage is computed as `(swap_used / swap_total) * 100.0`, or 0.0 if no swap exists.

use procfs::{Current, Meminfo};

/// Memory and swap utilization statistics.
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    /// Total physical RAM in bytes.
    pub total: u64,
    /// Currently used RAM in bytes (total - available).
    pub used: u64,
    /// Completely free/unused RAM in bytes.
    pub free: u64,
    /// Available RAM in bytes (includes reclaimable cache/buffer), for Linux semantics.
    pub available: u64,
    /// Total swap space in bytes.
    pub swap_total: u64,
    /// Free swap space in bytes.
    pub swap_free: u64,
    /// Swap usage as a percentage of total swap space (0.0–100.0).
    pub swap_used_percent: f32,
}

/// Collect current memory statistics.
///
/// Uses `procfs::Meminfo` to read `/proc/meminfo` and report RAM and swap utilization.
/// All byte values are in bytes. `swap_used` is a percentage (0.0–100.0) representing
/// how much of total swap space is currently in use; returns 0.0 if no swap partition exists.
///
/// ## Memory Calculation Notes
///
/// **Used memory** = total - available (where available includes reclaimable cache/buffer).
/// This matches standard Linux semantics where "used" includes reclaimable cache/buffer memory.
pub fn collect() -> MemoryStats {
    let m = match Meminfo::current() {
        Ok(mem) => mem,
        Err(_) => return MemoryStats::default(),
    };

    // procfs Meminfo fields are in bytes on this platform — no conversion needed.
    let total = m.mem_total;
    let available = m.mem_available.unwrap_or(m.mem_free);
    let used = total.saturating_sub(available);
    let free = m.mem_free;

    let swap_total = m.swap_total;
    let swap_free = m.swap_free;
    let swap_used_percent = if swap_total > 0 {
        ((swap_total.saturating_sub(swap_free)) as f64 / swap_total as f64 * 100.0) as f32
    } else {
        0.0
    };

    MemoryStats {
        total,
        used,
        free,
        available,
        swap_total,
        swap_free,
        swap_used_percent,
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
        assert!(stats.swap_used_percent >= 0.0 && stats.swap_used_percent <= 100.0);
    }

    /// Total memory must be positive on any running Linux system.
    #[test]
    fn test_total_memory_positive() {
        let stats = collect();
        assert!(stats.total > 0);
    }
}
