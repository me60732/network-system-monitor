//! # MetricsAggregator — packs metrics-core collection results into MetricPacket format
//!
//! Calls [`metrics_core::collect_all()`] to gather CPU, memory, disk, network, uptime, GPU, and
//! temperature stats in a single pass (< 50ms target), then transforms them into the flat-field
//! `MetricPacket` struct suitable for rkyv serialization and UDP transmission.
//!
//! ## Data Transformation Pipeline
//!
//! ```text
//! metrics-core::collect_all() → (CpuStats, MemoryStats, DiskStats, NetworkStats, UptimeStats, GpuStats, TemperatureStats)
//!   ↓ aggregate(machine_id, sequence_counter)
//! MetricPacket { version, machine_id, timestamp, sequence, cpu_usage, memory_used_percent, disk_used_percent, network_rx_bytes, uptime_seconds, gpu_vram_used_mb, temperature_celsius }
//! ```

use crate::config::ServiceConfig;
use crate::packet::MetricPacket;
use metrics_core::{collect_all, InterfaceStat};
use std::time::{SystemTime, UNIX_EPOCH};

/// Aggregator that collects system metrics and packs them into [`MetricPacket`] format.
pub struct MetricsAggregator {
    /// Service configuration providing machine_id for packet identity.
    config: ServiceConfig,
}

impl MetricsAggregator {
    /// Create a new aggregator with the given service configuration.
    pub fn new(config: ServiceConfig) -> Self {
        MetricsAggregator { config }
    }

    /// Collect all metrics via `metrics_core::collect_all()` and pack into a [`MetricPacket`].
    ///
    /// Sets timestamp to current Unix seconds, machine_id from config, and leaves sequence/hmac_tag
    /// for the [`crate::udp_sender::UdpSender`] to fill in before transmission.
    pub fn aggregate(&self) -> MetricPacket {
        // Collect all metrics in a single pass (performance target: < 50ms).
        let (cpu, memory, disk, network, uptime, gpu, temperature) = collect_all();

        // --- CPU usage — direct percentage from CpuStats::usage (0.0–100.0) ---
        let cpu_usage = cpu.usage;

        // --- Memory used percent — (used / total) * 100, guard against division by zero ---
        let memory_used_percent = if memory.total > 0 {
            ((memory.used as f64) / (memory.total as f64) * 100.0) as f32
        } else {
            0.0
        };

        // --- Disk used percent — sum(used) / sum(total) across all non-virtual partitions ---
        let disk_total: u64 = disk.partitions.iter().map(|p| p.total).sum();
        let disk_used: u64 = disk.partitions.iter().map(|p| p.used).sum();
        let disk_used_percent = if disk_total > 0 {
            ((disk_used as f64) / (disk_total as f64) * 100.0) as f32
        } else {
            0.0
        };

        // --- Network RX bytes — sum of rx_bytes across all non-loopback interfaces; fallback to loopback ---
        let network_rx_bytes = Self::primary_interface_rx(&network.interfaces);

        // --- Uptime seconds — direct from UptimeStats ---
        let uptime_seconds = uptime.seconds;

        // --- GPU VRAM used in MB — convert bytes to megabytes if available ---
        let gpu_vram_used_mb = match &gpu.vram_used {
            Some(bytes) => Some((bytes / 1_048_576) as u32),
            None => None,
        };

        // --- Temperature — prefer CPU temp; fallback to GPU temp if CPU is unavailable ---
        let temperature_celsius = match (temperature.cpu_temp, temperature.gpu_temp) {
            (Some(cpu_t), _) => Some(cpu_t),
            (None, Some(gpu_t)) => Some(gpu_t),
            (None, None) => None,
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Encode machine_id as fixed-length [u8; 20] — null-padded if shorter.
        let mut machine_id_bytes = [0u8; 20];
        let src = self.config.machine_id.as_bytes();
        let len = src.len().min(20);
        machine_id_bytes[..len].copy_from_slice(&src[..len]);

        MetricPacket {
            version: crate::packet::PROTOCOL_VERSION,
            machine_id: machine_id_bytes,
            timestamp,
            sequence: 0, // Filled by UdpSender before transmission
            cpu_usage,
            memory_used_percent,
            disk_used_percent,
            network_rx_bytes,
            uptime_seconds,
            disk_read_bytes: None,      // Phase 2: IO stats (sysinfo doesn't expose these)
            disk_write_bytes: None,     // Phase 2: IO stats (sysinfo doesn't expose these)
            network_rx_packets: None,   // Phase 2: packet counters (sysinfo doesn't expose these)
            network_tx_packets: None,   // Phase 2: packet counters (sysinfo doesn't expose these)
            network_rx_dropped: None,   // Phase 2: dropped packets (sysinfo doesn't expose these)
            network_tx_dropped: None,   // Phase 2: dropped packets (sysinfo doesn't expose these)
            memory_swap_used_pct: memory.swap_used, // Phase 2: swap usage percentage from MemoryStats
            gpu_vram_used_mb,
            temperature_celsius,
            hmac_tag: [0u8; 32], // Filled by UdpSender::send before transmission
        }
    }

    /// Find the primary non-loopback interface and return its cumulative rx_bytes.
    /// Falls back to loopback if no non-loopback interface is found.
    fn primary_interface_rx(interfaces: &[InterfaceStat]) -> u64 {
        // Prefer the first non-loopback interface with a name that isn't "lo".
        for iface in interfaces {
            if !iface.name.is_empty() && iface.name != "lo" {
                return iface.rx_bytes;
            }
        }

        // Fallback: use loopback ("lo") if no other interface is found.
        for iface in interfaces {
            if iface.name == "lo" {
                return iface.rx_bytes;
            }
        }

        0
    }

    /// Return the configured machine_id for this service instance.
    pub fn machine_id(&self) -> &str {
        &self.config.machine_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Aggregated packet contains correct machine ID (Beverly writes after implementation).
    #[test]
    fn test_aggregate_returns_machine_id() {
        let config = ServiceConfig::default();
        let aggregator = MetricsAggregator::new(config);

        let packet = aggregator.aggregate();
        
        // machine_id is [u8; 20] null-padded — convert to string for comparison.
        let len = packet.machine_id.iter().position(|&b| b == 0).unwrap_or(20);
        let packet_machine_id: &str = std::str::from_utf8(&packet.machine_id[..len]).unwrap();
        
        assert_eq!(packet_machine_id, aggregator.machine_id());
    }

    /// Aggregated packet has a valid timestamp (Beverly writes after implementation).
    #[test]
    fn test_aggregate_timestamp_valid() {
        let config = ServiceConfig::default();
        let aggregator = MetricsAggregator::new(config);

        let packet = aggregator.aggregate();
        assert!(packet.timestamp > 0, "timestamp should be set to current Unix time");
    }
}