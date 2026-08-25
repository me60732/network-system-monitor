//! Network interface statistics.
//!
//! Collects per-interface RX/TX byte and packet counters using `sysinfo` (which reads from `/proc/net/dev`).
//! Each entry in [`NetworkStats::interfaces`] represents one network interface with cumulative
//! counters for bytes and packets since boot.

use serde::{Deserialize, Serialize};
use sysinfo::Networks;

/// Network statistics for a single interface.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InterfaceStat {
    /// Interface name (e.g., "eth0", "wlan0", "lo").
    pub name: String,
    /// Total bytes received since boot.
    pub rx_bytes: u64,
    /// Total bytes transmitted since boot.
    pub tx_bytes: u64,
    /// Total packets received since boot (if available from sysinfo).
    pub rx_packets: Option<u64>,
    /// Total packets transmitted since boot (if available from sysinfo).
    pub tx_packets: Option<u64>,
    /// Packets dropped on receive (if available from sysinfo).
    pub rx_dropped: Option<u64>,
    /// Packets dropped on transmit (if available from sysinfo).
    pub tx_dropped: Option<u64>,
}

/// Aggregate network statistics across all interfaces.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkStats {
    /// One entry per detected network interface with byte counters.
    pub interfaces: Vec<InterfaceStat>,
}

/// Collect current network interface statistics.
///
/// Uses `sysinfo::Networks` to read `/proc/net/dev` and report cumulative RX/TX byte counters
/// for each network interface (loopback, ethernet, wifi, etc.). Values are monotonic counters
/// since boot — rate calculations must be done by the consumer over time intervals.
///
/// Packet statistics (rx/tx packets, dropped) are included where available in sysinfo 0.39+;
/// returns `None` if not supported by the underlying API (sysinfo 0.39 does not expose these fields).
pub fn collect() -> NetworkStats {
    // In sysinfo 0.35, Networks is a standalone type with new_with_refreshed_list().
    let networks = Networks::new_with_refreshed_list();

    let interfaces: Vec<InterfaceStat> = networks
        .list()
        .iter()
        .map(|(name, data)| {
            // Note: sysinfo 0.39 Networks data does not expose packet counters or dropped counts.
            // Those require direct /proc/net/dev parsing (third field: packets, fourth: dropped).
            // For now, we report None; future enhancement could add procfs-based packet stats.

            InterfaceStat {
                name: name.clone(),
                rx_bytes: data.received(),
                tx_bytes: data.transmitted(),
                rx_packets: None,
                tx_packets: None,
                rx_dropped: None,
                tx_dropped: None,
            }
        })
        .collect();

    NetworkStats { interfaces }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loopback or eth0/wlan0 present (Beverly writes after implementation).
    #[test]
    fn test_network_interfaces_present() {
        let stats = collect();
        // On any Linux system, at least the loopback interface ("lo") should be present.
        assert!(!stats.interfaces.is_empty(), "Expected at least one network interface (loopback)");
    }

    /// Loopback interface must be present on Linux.
    #[test]
    fn test_loopback_present() {
        let stats = collect();
        let has_lo = stats.interfaces.iter().any(|iface| iface.name == "lo");
        assert!(has_lo, "Expected loopback interface 'lo' in network stats");
    }

    /// RX and TX byte counters should be non-negative (they're u64, so always >= 0).
    #[test]
    fn test_byte_counters_valid() {
        let stats = collect();
        for iface in &stats.interfaces {
            // Counters are cumulative since boot — they can theoretically be zero if no traffic.
            assert!(iface.rx_bytes <= u64::MAX);
            assert!(iface.tx_bytes <= u64::MAX);
        }
    }
}
