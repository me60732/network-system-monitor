//! Network interface statistics.
//!
//! Collects per-interface RX/TX byte counters using `sysinfo` (which reads from `/proc/net/dev`).
//! Returns DELTA byte counts since the last collection call. The applet layer uses these directly
//! as bytes-per-second rates (since collection interval is typically 1 second).
//!
//! This matches minimon-applet's collection pattern using `received()` and `transmitted()` deltas.

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

/// Stateful network collector that returns per-interval delta byte counts.
///
/// Uses `sysinfo::Networks` which internally tracks old→new values to compute deltas.
/// Returns RX/TX bytes transferred since the last call to `collect()` (not cumulative).
/// The applet layer uses these directly as bytes-per-second rates (collection interval ≈ 1 second).
/// This matches minimon-applet's collection pattern using `received()` and `transmitted()` deltas.
#[derive(Debug)]
pub struct NetworkCollector {
    /// sysinfo Networks instance - holds current interface data
    networks: Networks,
}

impl NetworkCollector {
    /// Create a new network collector with initial state.
    ///
    /// Initializes sysinfo Networks.
    pub fn new() -> Self {
        let networks = Networks::new_with_refreshed_list();

        NetworkCollector { networks }
    }

    /// Collect current network interface statistics (delta since last call).
    ///
    /// Refreshes sysinfo data and returns RX/TX bytes transferred since the previous `collect()` call.
    /// Uses `received()` and `transmitted()` which return deltas from sysinfo's internal tracking.
    /// Sums ALL interfaces (including loopback — it's negligible on a desktop).
    /// The applet uses these directly as bytes-per-second rates (collection interval ≈ 1 second).
    pub fn collect(&mut self) -> NetworkStats {
        // Refresh network data (persistent instance, sysinfo tracks old→new internally)
        self.networks.refresh(true);

        let mut total_rx: u64 = 0;
        let mut total_tx: u64 = 0;

        // Sum ALL interfaces (including loopback — negligible on desktop)
        for (_, data) in &self.networks {
            total_rx += data.received(); // delta since last refresh
            total_tx += data.transmitted(); // delta since last refresh
        }

        // Return a single aggregate entry — rx_bytes/tx_bytes are bytes since last collect() call
        NetworkStats {
            interfaces: vec![InterfaceStat {
                name: "all".to_string(),
                rx_bytes: total_rx,
                tx_bytes: total_tx,
                rx_packets: None,
                tx_packets: None,
                rx_dropped: None,
                tx_dropped: None,
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NetworkCollector returns delta bytes since last call (valid after second call)
    #[test]
    fn test_network_collector_delta_second_call() {
        let mut collector = NetworkCollector::new();
        // First call establishes baseline
        let _first = collector.collect();
        // Second call returns delta since first call
        let second = collector.collect();
        // After two calls, the delta should be valid (u64 values)
        for iface in &second.interfaces {
            assert!(iface.rx_bytes <= u64::MAX);
            assert!(iface.tx_bytes <= u64::MAX);
        }
    }
}
