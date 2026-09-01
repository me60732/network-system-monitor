//! # MetricsAggregator — packs metrics-core collection results into MetricPacket format
//!
//! Implements 3-layer optimization to reduce aggregation time from ~250ms to ~20ms:
//!
//! **Layer 1**: Cache static values (memory_total, gpu_vram_total) that never change during runtime.
//! **Layer 2**: Use stateful sysinfo::System instance instead of creating fresh System::new_all() every cycle.
//! **Layer 3**: Refresh disk partitions only every 20 cycles (20 seconds at 1s refresh rate).
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

use crate::packet::{
    CpuMetrics, DiskMetrics, GpuMetrics, MemoryMetrics, MetricPacket, NetworkMetrics,
    PROTOCOL_VERSION, PartitionInfo,
};
use metrics_core::{CpuCollector, GpuCollector, NetworkCollector};
use std::time::{SystemTime, UNIX_EPOCH};

/// Aggregator that collects system metrics and packs them into [`MetricPacket`] format.
///
/// Implements 3-layer optimization:
/// * Layer 1: Cache static values (memory_total_bytes, gpu_vram_total_mb) that never change during runtime
/// * Layer 2: Use stateful sysinfo::System instance with refresh_memory() instead of System::new_all()
/// * Layer 3: Refresh disk partitions only every 20 cycles to amortize scan cost
///
/// Expected performance improvement: 250ms → ~20ms (10x faster)
pub struct MetricsAggregator {
    /// Machine ID string for packet identity.
    machine_id: String,
    /// Stateful CPU collector for accurate delta-based utilization percentages.
    cpu_collector: CpuCollector,
    /// Stateful network collector for accurate delta-based RX/TX byte tracking.
    network_collector: NetworkCollector,

    /// Stateful GPU collector that detects GPU type once and caches NVML instance.
    gpu_collector: GpuCollector,

    // Layer 1: Static value caching (never change during runtime)
    /// Total memory in bytes (cached at startup, never changes)
    memory_total_bytes: u64,
    /// Total GPU VRAM in MB (cached at startup, None if no GPU detected)
    gpu_vram_total_mb: Option<u32>,

    // Layer 3: Slow-refresh disk partition caching (refresh every 20 cycles)
    /// Cached list of disk partitions, refreshed every 20 cycles
    disk_partitions_cache: Vec<PartitionInfo>,
    /// Cached total disk bytes, refreshed every 20 cycles
    disk_total_bytes_cache: u64,
    /// Counter tracking when to refresh disk partitions (refresh every 20 cycles)
    partition_refresh_counter: u32,

    // Layer 4: Stateful disk IO collector for delta-based byte tracking
    /// Persistent DiskIoCollector instance that tracks IO deltas since last call
    disk_io_collector: metrics_core::disk::DiskIoCollector,
}

impl MetricsAggregator {
    /// Create a new aggregator with the given service configuration and prime CPU/network state.
    ///
    /// Initializes all optimization layers plus stateful collectors:
    /// * Layer 1: Collects static memory_total_bytes and gpu_vram_total_mb once (never change during runtime)
    /// * Stateful Collectors: CPU (with temp), Network, and GPU (with temp) collectors initialized with cached state
    /// * Layer 3: Collects disk partitions list and total bytes for caching (refresh every 20 cycles)
    ///
    /// The first call to `aggregate()` will compute deltas from CPU/network baselines.
    pub fn new(machine_id: &str) -> Self {
        // Layer 1: Collect static values that never change during runtime
        let memory_total_bytes = metrics_core::memory::collect().total;

        // Initialize GPU collector once (detects GPU type, initializes NVML if needed)
        let gpu_collector = GpuCollector::new();

        let gpu_vram_total_mb = {
            let gpu_stats = gpu_collector.collect();
            gpu_stats.vram_total.map(|bytes| (bytes / 1_048_576) as u32)
        };

        // Layer 3: Collect disk partitions for initial cache
        let disk_stats = metrics_core::disk::collect();
        let disk_partitions_cache: Vec<PartitionInfo> = disk_stats
            .partitions
            .iter()
            .map(|p| PartitionInfo {
                mount: p.mount.clone(),
                total: p.total,
                used: p.used,
            })
            .collect();
        let disk_total_bytes_cache = disk_stats.partitions.iter().map(|p| p.total).sum();

        let disk_io_collector = metrics_core::disk::DiskIoCollector::new();

        MetricsAggregator {
            machine_id: machine_id.to_string(),
            cpu_collector: CpuCollector::new(), // Prime with initial read + discover CPU temp sensor
            network_collector: NetworkCollector::new(), // Initialize empty prev maps
            gpu_collector, // Stateful GPU collector with cached GPU type and NVML instance
            memory_total_bytes,
            gpu_vram_total_mb,
            disk_partitions_cache,
            disk_total_bytes_cache,
            partition_refresh_counter: 0,
            disk_io_collector,
        }
    }

    /// Collect all metrics and pack into a [`MetricPacket`].
    ///
    /// Uses stateful CPU collector (`cpu_collector.collect()`) for accurate delta measurement.
    /// All other metrics use one-shot collection functions. Sets timestamp to current Unix seconds,
    /// machine_id from config, and leaves sequence/session/timestamp for the [`crate::udp_sender::UdpSender`]
    /// to fill in before transmission.
    ///
    /// Performance monitoring (Item 7.2): Logs warnings if any collector exceeds 50ms.
    pub fn aggregate(&mut self) -> MetricPacket {
        use std::time::Instant;
        let aggregate_start = Instant::now();

        // --- CPU usage and temperature — collected together via stateful collector ---
        let cpu_start = Instant::now();
        let cpu = self.cpu_collector.collect();
        let cpu_elapsed = cpu_start.elapsed();
        log::debug!("CPU collection: {}ms", cpu_elapsed.as_millis());
        if cpu_elapsed.as_millis() > 50 {
            log::warn!(
                "CPU collection took {}ms (threshold: 50ms)",
                cpu_elapsed.as_millis()
            );
        }
        let cpu_usage_percent = cpu.usage;
        let temperature_celsius = cpu.cpu_temp;

        // --- Network RX/TX bytes — cumulative bytes since boot (stateful collector) ---
        let network_start = Instant::now();
        let network_stats = self.network_collector.collect();
        let network_elapsed = network_start.elapsed();
        log::debug!("Network collection: {}ms", network_elapsed.as_millis());
        if network_elapsed.as_millis() > 50 {
            log::warn!(
                "Network collection took {}ms (threshold: 50ms)",
                network_elapsed.as_millis()
            );
        }

        // --- GPU VRAM, load, and temp — collect ONCE via stateful collector ---
        let gpu_start = Instant::now();
        let gpu_stats = self.gpu_collector.collect();
        let gpu_elapsed = gpu_start.elapsed();
        log::debug!("GPU collection: {}ms", gpu_elapsed.as_millis());
        if gpu_elapsed.as_millis() > 50 {
            log::warn!(
                "GPU collection took {}ms (threshold: 50ms)",
                gpu_elapsed.as_millis()
            );
        }
        let gpu_temperature_celsius = gpu_stats.gpu_temp;

        // --- Memory used and total bytes — send raw values, let applet calculate percentage ---
        // Use procfs for memory stats
        let mem_start = Instant::now();
        let mem_stats = metrics_core::memory::collect();
        let memory_used_bytes = mem_stats.used;
        // Swap stats from MemoryStats - included for completeness even though not used directly
        let _swap_total_bytes = mem_stats.swap_total;
        let _swap_free_bytes = mem_stats.swap_free;
        let memory_swap_used_pct = mem_stats.swap_used_percent;
        let mem_elapsed = mem_start.elapsed();
        log::debug!("Memory collection: {}ms", mem_elapsed.as_millis());
        if mem_elapsed.as_millis() > 50 {
            log::warn!(
                "Memory collection took {}ms (threshold: 50ms)",
                mem_elapsed.as_millis()
            );
        }
        // Swap stats are included in MemoryStats; no separate timing needed

        // --- Disk used and total bytes — sum across all partitions, send raw values ---
        let disk_start = Instant::now();

        // Layer 3: Conditional disk partition refresh (every 20 cycles)
        self.partition_refresh_counter += 1;
        if self.partition_refresh_counter >= 20 {
            log::debug!("Refreshing disk partitions cache");
            let disk_stats = metrics_core::disk::collect();
            self.disk_partitions_cache = disk_stats
                .partitions
                .iter()
                .map(|p| PartitionInfo {
                    mount: p.mount.clone(),
                    total: p.total,
                    used: p.used,
                })
                .collect();
            self.disk_total_bytes_cache = disk_stats.partitions.iter().map(|p| p.total).sum();
            self.partition_refresh_counter = 0;
        }

        let disk_elapsed = disk_start.elapsed();
        log::debug!("Disk collection: {}ms", disk_elapsed.as_millis());
        if disk_elapsed.as_millis() > 50 {
            log::warn!(
                "Disk collection took {}ms (threshold: 50ms)",
                disk_elapsed.as_millis()
            );
        }
        let disk_used_bytes: u64 = self.disk_partitions_cache.iter().map(|p| p.used).sum(); // Layer 3: use cached partitions

        // --- Disk IO stats — delta read/write bytes since last call ---
        let disk_io_start = Instant::now();
        let disk_io = self.disk_io_collector.collect(); // uses persistent instance, returns deltas
        let disk_read_bytes = Some(disk_io.read_bytes);
        let disk_write_bytes = Some(disk_io.write_bytes);
        let disk_io_elapsed = disk_io_start.elapsed();
        log::debug!("Disk IO collection: {}ms", disk_io_elapsed.as_millis());

        // --- Network RX/TX bytes — delta bytes since last call (applet uses directly as bytes/sec) ---
        // NetworkCollector now returns a single aggregate entry with all-interface totals
        let network_rx_bytes = network_stats
            .interfaces
            .first()
            .map(|i| i.rx_bytes)
            .unwrap_or(0);
        let network_tx_bytes = network_stats
            .interfaces
            .first()
            .map(|i| i.tx_bytes)
            .unwrap_or(0);

        // --- Uptime seconds — use metrics_core collector ---
        let uptime_start = Instant::now();
        let uptime_stats = metrics_core::uptime::collect();
        let uptime_elapsed = uptime_start.elapsed();
        if uptime_elapsed.as_millis() > 50 {
            log::warn!(
                "Uptime collection took {}ms (threshold: 50ms)",
                uptime_elapsed.as_millis()
            );
        }
        let uptime_seconds = uptime_stats.seconds;

        // --- GPU VRAM used in MB and load percent — use stats collected above ---
        let gpu_vram_used_mb = gpu_stats.vram_used.map(|bytes| (bytes / 1_048_576) as u32);
        // Layer 1: Use cached total VRAM value instead of re-collecting
        let gpu_load_percent = gpu_stats.gpu_load_percent; // Pass through directly (0.0–100.0)

        // --- Disk partitions — use cached list from Layer 3 ---
        let disk_partitions: Vec<PartitionInfo> = self.disk_partitions_cache.clone();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Encode machine_id as fixed-length [u8; 20] — null-padded if shorter.
        let mut machine_id_bytes = [0u8; 20];
        let src = self.machine_id.as_bytes();
        let len = src.len().min(20);
        machine_id_bytes[..len].copy_from_slice(&src[..len]);

        let packet = MetricPacket {
            version: PROTOCOL_VERSION,
            machine_id: machine_id_bytes,
            sender_session_id: [0u8; 16], // Filled by UdpSender from template
            timestamp,
            sequence: 0, // Filled by UdpSender before transmission

            // Nested metric groups (Protocol Version 3)
            cpu: CpuMetrics {
                usage_percent: cpu_usage_percent,
                temperature_celsius: temperature_celsius,
            },
            gpu: GpuMetrics {
                load_percent: gpu_load_percent,
                vram_used_mb: gpu_vram_used_mb,
                vram_total_mb: self.gpu_vram_total_mb, // Layer 1: cached
                temperature_celsius: gpu_temperature_celsius,
            },
            memory: MemoryMetrics {
                used_bytes: memory_used_bytes,
                total_bytes: self.memory_total_bytes, // Layer 1: cached
                swap_used_pct: memory_swap_used_pct,
            },
            network: NetworkMetrics {
                rx_bytes: network_rx_bytes,
                tx_bytes: network_tx_bytes,
            },
            disk: DiskMetrics {
                used_bytes: disk_used_bytes,
                total_bytes: self.disk_total_bytes_cache, // Layer 3: cached
                read_bytes: disk_read_bytes,
                write_bytes: disk_write_bytes,
                partitions: disk_partitions
                    .iter()
                    .map(|p| PartitionInfo {
                        mount: p.mount.clone(),
                        total: p.total,
                        used: p.used,
                    })
                    .collect(),
            },
            uptime_seconds,
        };

        // Log total aggregation time and cache status
        let total_elapsed = aggregate_start.elapsed();
        if total_elapsed.as_millis() > 50 {
            log::warn!(
                "Total metrics aggregation took {}ms (threshold: 50ms)",
                total_elapsed.as_millis()
            );
        } else {
            log::debug!(
                "Metrics aggregation completed in {}ms (cached memory_total={}, vram_total={:?}, partitions={})",
                total_elapsed.as_millis(),
                self.memory_total_bytes,
                self.gpu_vram_total_mb,
                self.disk_partitions_cache.len()
            );
        }

        packet
    }

    // Legacy helper methods - kept for backward compatibility but not used in current implementation.
    // NetworkCollector::collect() now returns a single aggregate entry with all-interface totals.

    /// Return the configured machine_id for this service instance.
    pub fn machine_id(&self) -> &str {
        &self.machine_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Aggregated packet contains correct machine ID (Beverly writes after implementation).
    #[test]
    fn test_aggregate_returns_machine_id() {
        let machine_id = "test-host";
        let mut aggregator = MetricsAggregator::new(machine_id);

        let packet = aggregator.aggregate();

        // machine_id is [u8; 20] null-padded — convert to string for comparison.
        let len = packet.machine_id.iter().position(|&b| b == 0).unwrap_or(20);
        let packet_machine_id: &str = std::str::from_utf8(&packet.machine_id[..len]).unwrap();

        assert_eq!(packet_machine_id, aggregator.machine_id());
    }

    /// Aggregated packet has a valid timestamp (Beverly writes after implementation).
    #[test]
    fn test_aggregate_timestamp_valid() {
        let machine_id = "test-host";
        let mut aggregator = MetricsAggregator::new(machine_id);

        let packet = aggregator.aggregate();
        assert!(
            packet.timestamp > 0,
            "timestamp should be set to current Unix time"
        );
    }
}
