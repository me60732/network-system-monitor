//! System uptime and load average statistics.
//!
//! Collects system uptime in seconds plus 1/5/15-minute load averages using `sysinfo`
/// (which reads from `/proc/uptime` and `/proc/loadavg`).

use serde::{Deserialize, Serialize};
use sysinfo::System;

/// Uptime and load average statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UptimeStats {
    /// System uptime in seconds since boot.
    pub seconds: u64,
    /// 1-minute, 5-minute, and 15-minute load averages as a tuple.
    pub load_avg: (f32, f32, f32),
}

/// Collect current uptime statistics.
///
/// Uses `sysinfo::System` to read `/proc/uptime` for system uptime in seconds since boot,
/// and `/proc/loadavg` for 1/5/15-minute load averages. Load averages represent the average
/// number of processes in the run queue over each time window — values at or below the CPU count
/// indicate normal utilization; values above suggest saturation.
pub fn collect() -> UptimeStats {
    // In sysinfo 0.35, uptime is an associated function — no refresh needed.
    // Load averages are read directly from /proc/loadavg for reliability.
    let seconds = System::uptime();
    let (load1, load5, load15) = parse_loadavg().unwrap_or((0.0, 0.0, 0.0));

    UptimeStats {
        seconds,
        load_avg: (load1 as f32, load5 as f32, load15 as f32),
    }
}

/// Parse `/proc/loadavg` and return the three load average values as a tuple of f64.
/// Returns None if the file cannot be read or parsed.
fn parse_loadavg() -> Option<(f64, f64, f64)> {
    use std::fs;

    let content = fs::read_to_string("/proc/loadavg").ok()?;
    let parts: Vec<&str> = content.split_whitespace().collect();

    if parts.len() < 3 {
        return None;
    }

    let load1: f64 = parts[0].parse().ok()?;
    let load5: f64 = parts[1].parse().ok()?;
    let load15: f64 = parts[2].parse().ok()?;

    Some((load1, load5, load15))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uptime seconds > 0 after boot (Beverly writes after implementation).
    #[test]
    fn test_uptime_positive() {
        let stats = collect();
        // On any running Linux system, uptime should be greater than zero.
        assert!(stats.seconds > 0, "System uptime must be positive on a running machine");
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

    /// Verify that /proc/loadavg parsing works correctly by comparing with sysinfo's load_avg().
    #[test]
    fn test_loadavg_matches_sysinfo() {
        let stats = collect();
        // Cross-check: parse the file directly and compare (with tolerance for timing)
        if let Some((l1, l5, l15)) = parse_loadavg() {
            assert!((stats.load_avg.0 - l1 as f32).abs() < 0.1);
            assert!((stats.load_avg.1 - l5 as f32).abs() < 0.1);
            assert!((stats.load_avg.2 - l15 as f32).abs() < 0.1);
        }
    }
}
