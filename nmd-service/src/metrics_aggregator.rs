//! # MetricsAggregator — packs metrics-core collection results into MetricPacket format
//!
//! Calls [`metrics_core::CpuCollector`] for stateful CPU delta measurement,
//! [`metrics_core::NetworkCollector`] for stateful network delta tracking, and other collectors
//! for one-shot metrics (memory, disk, uptime, GPU, temperature). Packs all metrics into
//! the flat-field [`MetricPacketFlat`] struct suitable for rkyv serialization and UDP transmission.
//!
//! ## Data Transformation Pipeline
//!
//! ```text
//! CpuCollector::collect() → CpuStats { usage, cores }
//! NetworkCollector::collect() → NetworkStats { interfaces: Vec<InterfaceStat> } (deltas)
//! MemoryStats::collect() → MemoryStats { total, used, free, available, swap_used }
//! DiskStats::collect() → DiskStats { partitions: Vec<PartitionStat> }
//! UptimeStats::collect() → UptimeStats { seconds, load_avg }
//! GpuStats::collect() → GpuStats { vram_total, vram_used, gpu_load_percent }
//! TemperatureStats::collect() → TemperatureStats { cpu_temp, gpu_temp }
//!   ↓ aggregate(machine_id)
//! MetricPacketFlat { version, machine_id, timestamp, sequence, cpu_usage_percent, memory_used_bytes, ... }
//! ```
//!
//! ## Design Philosophy
//!
//! The service collects and sends **raw metric values** (bytes, counts, etc.). The desktop applet
//! handles all presentation logic including percentage calculations. This separation allows users to
//! configure display preferences (percentages vs. absolute values) without changing the service.

use crate::config::ServiceConfig;
use crate::packet_flat::{MetricPacketFlat, PROTOCOL_VERSION_FLAT};
use metrics_core::{CpuCollector, InterfaceStat, NetworkCollector};
use std::time::{SystemTime, UNIX_EPOCH};

/// Aggregator that collects system metrics and packs them into [`MetricPacket`] format.
///
/// Uses stateful CPU collector (`metrics_core::CpuCollector`) for accurate delta measurement
/// via the prev/current pattern. All other metrics use one-shot collection functions.
pub struct MetricsAggregator {
    /// Service configuration providing machine_id for packet identity.
    config: ServiceConfig,
    /// Stateful CPU collector for accurate delta-based utilization percentages.
    cpu_collector: CpuCollector,
    /// Stateful network collector for accurate delta-based RX/TX byte tracking.
    network_collector: NetworkCollector,
}

impl MetricsAggregator {
    /// Create a new aggregator with the given service configuration and prime CPU/network state.
    ///
    /// Initializes [`CpuCollector`] which performs an initial /proc/stat read to establish
    /// the baseline state, and [`NetworkCollector`] which initializes empty previous value maps.
    /// The first call to `aggregate()` will compute deltas from these baselines.
    pub fn new(config: ServiceConfig) -> Self {
        MetricsAggregator {
            config,
            cpu_collector: CpuCollector::new(),  // Prime with initial read
            network_collector: NetworkCollector::new(),  // Initialize empty prev maps
        }
    }

    /// Collect all metrics and pack into a [`MetricPacketFlat`].
    ///
    /// Uses stateful CPU collector (`cpu_collector.collect()`) for accurate delta measurement.
    /// All other metrics use one-shot collection functions. Sets timestamp to current Unix seconds,
    /// machine_id from config, and leaves sequence/hmac_tag for the [`crate::udp_sender::UdpSender`]
    /// to fill in before transmission.
    pub fn aggregate(&mut self) -> MetricPacketFlat {
        // Collect CPU stats via stateful collector (delta measurement)
        let cpu = self.cpu_collector.collect();
        
        // Collect network stats via stateful collector (cumulative bytes since boot)
        let network_stats = self.network_collector.collect();

        // --- CPU usage — direct percentage from CpuStats::usage (0.0–100.0) ---
        let cpu_usage_percent = cpu.usage;

        // --- Temperature — collect both CPU and GPU temps separately ---
        let temp_stats = metrics_core::temperature::collect();
        let temperature_celsius = temp_stats.cpu_temp;
        let gpu_temperature_celsius = temp_stats.gpu_temp;

        // --- Memory used and total bytes — send raw values, let applet calculate percentage ---
        // Use sysinfo for memory stats (it reads /proc/meminfo)
        let sys = sysinfo::System::new_all();
        let memory_used_bytes = sys.total_memory().saturating_sub(sys.available_memory());
        let memory_total_bytes = sys.total_memory();

        // --- Swap usage percentage ---
        let memory_swap_used_pct = metrics_core::memory::collect().swap_used;

        // --- Disk used and total bytes — sum across all partitions, send raw values ---
        let disk_stats = metrics_core::disk::collect();
        let disk_total_bytes: u64 = disk_stats.partitions.iter().map(|p| p.total).sum();
        let disk_used_bytes: u64 = disk_stats.partitions.iter().map(|p| p.used).sum();

        // --- Disk IO stats — cumulative read/write bytes since boot ---
        let disk_io = metrics_core::disk::collect_io();
        let disk_read_bytes = Some(disk_io.read_bytes);
        let disk_write_bytes = Some(disk_io.write_bytes);

        // --- Network RX/TX bytes — cumulative bytes since boot (applet computes rates) ---
        let network_interfaces: Vec<InterfaceStat> = network_stats.interfaces;
        let network_rx_bytes = Self::primary_interface_rx(&network_interfaces);
        let network_tx_bytes = Self::primary_interface_tx(&network_interfaces);

        // --- Uptime seconds — use metrics_core collector ---
        let uptime_stats = metrics_core::uptime::collect();
        let uptime_seconds = uptime_stats.seconds;

        // --- GPU VRAM used in MB and load percent — convert bytes to megabytes, pass load as-is ---
        let gpu_stats = metrics_core::gpu::collect();
        let gpu_vram_used_mb = gpu_stats.vram_used.map(|bytes| (bytes / 1_048_576) as u32);
        let gpu_vram_total_mb = gpu_stats.vram_total.map(|bytes| (bytes / 1_048_576) as u32);
        let gpu_load_percent = gpu_stats.gpu_load_percent; // Pass through directly (0.0–100.0)

        // --- Disk partitions — collect mount point, total, and used for each partition ---
        let disk_partitions: Vec<crate::packet::PartitionInfo> = disk_stats
            .partitions
            .iter()
            .map(|p| crate::packet::PartitionInfo {
                mount: p.mount.clone(),
                total: p.total,
                used: p.used,
            })
            .collect();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Encode machine_id as fixed-length [u8; 20] — null-padded if shorter.
        let mut machine_id_bytes = [0u8; 20];
        let src = self.config.machine_id.as_bytes();
        let len = src.len().min(20);
        machine_id_bytes[..len].copy_from_slice(&src[..len]);

        MetricPacketFlat {
            version: PROTOCOL_VERSION_FLAT, // Use flat protocol version
            machine_id: machine_id_bytes,
            timestamp,
            sequence: 0, // Filled by UdpSender before transmission
            
            // Flat metric fields (Phase 3: flat for UDP zero-copy mutation)
            cpu_usage_percent: cpu_usage_percent,
            cpu_temperature_celsius: temperature_celsius,
            
            gpu_load_percent: gpu_load_percent,
            gpu_vram_used_mb: gpu_vram_used_mb,
            gpu_vram_total_mb: gpu_vram_total_mb,
            gpu_temperature_celsius: gpu_temperature_celsius,
            
            memory_used_bytes: memory_used_bytes,
            memory_total_bytes: memory_total_bytes,
            memory_swap_used_pct: memory_swap_used_pct,
            
            network_rx_bytes: network_rx_bytes,
            network_tx_bytes: network_tx_bytes,
            
            disk_used_bytes: disk_used_bytes,
            disk_total_bytes: disk_total_bytes,
            disk_read_bytes: disk_read_bytes,
            disk_write_bytes: disk_write_bytes,
            // Convert PartitionInfo types
            disk_partitions: disk_partitions.iter().map(|p| crate::packet::PartitionInfo {
                mount: p.mount.clone(),
                total: p.total,
                used: p.used,
            }).collect(),
            
            uptime_seconds,
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

    /// Find the primary non-loopback interface and return its cumulative tx_bytes.
    /// Falls back to loopback if no non-loopback interface is found.
    fn primary_interface_tx(interfaces: &[InterfaceStat]) -> u64 {
        // Prefer the first non-loopback interface with a name that isn't "lo".
        for iface in interfaces {
            if !iface.name.is_empty() && iface.name != "lo" {
                return iface.tx_bytes;
            }
        }

        // Fallback: use loopback ("lo") if no other interface is found.
        for iface in interfaces {
            if iface.name == "lo" {
                return iface.tx_bytes;
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
        let mut aggregator = MetricsAggregator::new(config);

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
        let mut aggregator = MetricsAggregator::new(config);

        let packet = aggregator.aggregate();
        assert!(packet.timestamp > 0, "timestamp should be set to current Unix time");
    }
}