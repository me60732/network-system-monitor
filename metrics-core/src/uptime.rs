//! System uptime and load average statistics.
//!
//! Collects system uptime in seconds plus 1/5/15-minute load averages using `procfs`
//! (which reads from `/proc/uptime` and `/proc/loadavg`).

use procfs::{Current, LoadAverage, Uptime};

/// Uptime and load average statistics.
#[derive(Debug, Clone, Default)]
pub struct UptimeStats {
    /// System uptime in seconds since boot.
    pub seconds: u64,
    /// 1-minute, 5-minute, and 15-minute load averages as a tuple.
    pub load_avg: (f32, f32, f32),
}

/// Collect current uptime statistics.
///
/// Uses `procfs::Uptime` to read `/proc/uptime` for system uptime in seconds since boot,
/// and `procfs::LoadAverage` to read `/proc/loadavg` for 1/5/15-minute load averages.
/// Load averages represent the average number of processes in the run queue over each time window —
/// values at or below the CPU count indicate normal utilization; values above suggest saturation.
pub fn collect() -> UptimeStats {
    let seconds = Uptime::current().map(|u| u.uptime as u64).unwrap_or(0);

    let load_avg = LoadAverage::current()
        .map(|l| (l.one as f32, l.five as f32, l.fifteen as f32))
        .unwrap_or((0.0, 0.0, 0.0));

    UptimeStats { seconds, load_avg }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uptime seconds > 0 after boot (Beverly writes after implementation).
    #[test]
    fn test_uptime_positive() {
        let stats = collect();
        // On any running Linux system, uptime should be greater than zero.
        assert!(
            stats.seconds > 0,
            "System uptime must be positive on a running machine"
        );
    }

    /// Load averages should all be non-negative and load15 >= load5 >= load1 is not guaranteed
    /// but they should all be valid numbers (no NaN).
    #[test]
    fn test_load_avg_non_negative() {
        let stats = collect();
        assert!(!stats.load_avg.0.is_nan());
        assert!(!stats.load_avg.1.is_nan());
        assert!(!stats.load_avg.2.is_nan());
        // Load averages are always >= 0
        assert!(stats.load_avg.0 >= 0.0);
        assert!(stats.load_avg.1 >= 0.0);
        assert!(stats.load_avg.2 >= 0.0);
    }

    /// Verify that /proc/loadavg parsing works correctly by comparing with procfs.
    #[test]
    fn test_loadavg_matches_procfs() {
        let stats = collect();
        // Cross-check: read via procfs directly and compare (with tolerance for timing)
        if let Ok(load) = LoadAverage::current() {
            assert!((stats.load_avg.0 - load.one as f32).abs() < 0.1);
            assert!((stats.load_avg.1 - load.five as f32).abs() < 0.1);
            assert!((stats.load_avg.2 - load.fifteen as f32).abs() < 0.1);
        }
    }
}
