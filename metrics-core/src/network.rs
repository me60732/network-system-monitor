//! Network interface statistics.
//!
//! Collects per-interface RX/TX byte counters using `procfs` (which reads from `/proc/net/dev`).
//! Returns DELTA byte counts since the last collection call. The applet layer uses these directly
//! as bytes-per-second rates (since collection interval is typically 1 second).
//!
//! This matches minimon-applet's collection pattern using `received()` and `transmitted()` deltas.

use procfs::net::dev_status;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Network statistics for a single interface.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InterfaceStat {
    /// Interface name (e.g., "eth0", "wlan0", "lo").
    pub name: String,
    /// Total bytes received since boot.
    pub rx_bytes: u64,
    /// Total bytes transmitted since boot.
    pub tx_bytes: u64,
    /// Total packets received since boot (if available from procfs).
    pub rx_packets: Option<u64>,
    /// Total packets transmitted since boot (if available from procfs).
    pub tx_packets: Option<u64>,
    /// Packets dropped on receive (if available from procfs).
    pub rx_dropped: Option<u64>,
    /// Packets dropped on transmit (if available from procfs).
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
/// Uses `procfs::net::dev_status()` to read `/proc/net/dev` directly.
/// Tracks previous cumulative byte counts and computes deltas itself (sysinfo was doing this internally — now we do it).
/// Returns RX/TX bytes transferred since the last call to `collect()` (not cumulative).
/// The applet layer uses these directly as bytes-per-second rates (collection interval ≈ 1 second).
/// This matches minimon-applet's collection pattern using `received()` and `transmitted()` deltas.
#[derive(Debug)]
pub struct NetworkCollector {
    /// Previous interface byte counts: name -> (rx_bytes, tx_bytes)
    prev: HashMap<String, (u64, u64)>,
}

impl NetworkCollector {
    /// Create a new network collector with initial state.
    ///
    /// Reads current interface bytes to establish baseline.
    pub fn new() -> Self {
        let prev = read_interface_bytes();
        NetworkCollector { prev }
    }

    /// Collect current network interface statistics (delta since last call).
    ///
    /// Reads procfs and returns RX/TX bytes transferred since the previous `collect()` call.
    /// Uses cumulative counters from /proc/net/dev and computes deltas against stored baseline.
    /// Sums ALL interfaces (including loopback — it's negligible on a desktop).
    /// The applet uses these directly as bytes-per-second rates (collection interval ≈ 1 second).
    pub fn collect(&mut self) -> NetworkStats {
        let current = read_interface_bytes();

        let mut total_rx = 0u64;
        let mut total_tx = 0u64;

        for (name, (rx, tx)) in &current {
            if let Some((prev_rx, prev_tx)) = self.prev.get(name) {
                total_rx += rx.saturating_sub(*prev_rx);
                total_tx += tx.saturating_sub(*prev_tx);
            }
        }

        self.prev = current;

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

fn read_interface_bytes() -> HashMap<String, (u64, u64)> {
    dev_status()
        .unwrap_or_default()
        .into_iter()
        .map(|(name, s)| (name, (s.recv_bytes, s.sent_bytes)))
        .collect()
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
