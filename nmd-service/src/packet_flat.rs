//! # MetricPacketFlat — legacy flat structure for UDP zero-copy mutation compatibility

use rkyv::{Archive, Deserialize, Serialize};

/// Disk partition information (from packet.rs) - shared between nested and flat structures

/// A single metrics snapshot sent over UDP from remote machine → desktop applet.
///
/// **FLAT FIELD STRUCT** for rkyv zero-copy mutation compatibility via munge! API.
/// All fields except `hmac_tag` are included in the HMAC-SHA256 digest computed by
/// [`crate::udp_sender::UdpSender::send`]. The tag is verified on receipt and packets
/// failing verification or freshness checks (< 10s old) are silently dropped per Worf's spec.
///
/// **Protocol Version 3**: Maintains flat fields for zero-copy UDP transmission compatibility,
/// while nested structs are used in data model for better type safety.
pub const PROTOCOL_VERSION_FLAT: u32 = 3;

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct MetricPacketFlat {
    // ── Protocol Version & Security (Worf Phase 1A) ───────────────────────

    /// Protocol version for detecting incompatible sender/receiver versions.
    pub version: u32,

    /// Unique identifier for the sending machine (e.g., hostname or UUID).
    /// Stored as fixed-length [u8; 20] — null-padded if shorter than 20 bytes.
    pub machine_id: [u8; 20],

    /// Unix timestamp in seconds when this packet was assembled.
    pub timestamp: u64,

    /// Monotonic sequence counter incremented with every packet sent by this service instance.
    pub sequence: u32,

    // ── Flat Metric Fields (Phase 3: flat for zero-copy mutation) ─────────

    /// CPU usage percentage (0.0–100.0)
    pub cpu_usage_percent: f32,
    
    /// CPU temperature in Celsius — `None` if thermal sensors unavailable.
    pub cpu_temperature_celsius: Option<f32>,

    /// GPU utilization load percentage (0.0–100.0) — `None` if GPU monitoring unavailable.
    pub gpu_load_percent: Option<f32>,
    
    /// VRAM used in megabytes — `None` on systems without a discrete GPU.
    pub gpu_vram_used_mb: Option<u32>,
    
    /// VRAM total in megabytes — `None` on systems without a discrete GPU.
    pub gpu_vram_total_mb: Option<u32>,

    /// GPU junction temperature in Celsius — `None` if GPU thermal sensors unavailable.
    pub gpu_temperature_celsius: Option<f32>,

    /// Used memory in bytes.
    pub memory_used_bytes: u64,
    
    /// Total memory in bytes.
    pub memory_total_bytes: u64,
    
    /// Swap usage as a percentage of total swap space (0.0–100.0).
    pub memory_swap_used_pct: f32,

    /// Total bytes received on the primary network interface since boot.
    pub network_rx_bytes: u64,
    
    /// Total bytes transmitted on the primary network interface since boot.
    pub network_tx_bytes: u64,

    /// Used disk space in bytes (sum across all partitions).
    pub disk_used_bytes: u64,
    
    /// Total disk space in bytes (sum across all partitions).
    pub disk_total_bytes: u64,

    /// Total disk read bytes since boot — `None` if sysinfo doesn't expose IO stats.
    pub disk_read_bytes: Option<u64>,
    
    /// Total disk write bytes since boot — `None` if sysinfo doesn't expose IO stats.
    pub disk_write_bytes: Option<u64>,

    /// Disk partition information (mount point, total, used) for all mounted partitions
    pub disk_partitions: Vec<crate::packet::PartitionInfo>,

    /// System uptime in seconds since last boot.
    pub uptime_seconds: u64,

    // ── Authentication Tag (Worf Phase 1A) ────────────────────────────────

    /// HMAC-SHA256 tag over all fields above this one, computed by UdpSender before transmission.
    pub hmac_tag: [u8; 32],
}

impl MetricPacketFlat {
    /// Convert from nested [`MetricPacket`] to flat structure for UDP transmission.
    pub fn from_nested(packet: &crate::packet::MetricPacket) -> Self {
        MetricPacketFlat {
            version: packet.version,
            machine_id: packet.machine_id,
            timestamp: packet.timestamp,
            sequence: packet.sequence,
            
            cpu_usage_percent: packet.cpu.usage_percent,
            cpu_temperature_celsius: packet.cpu.temperature_celsius,
            
            gpu_load_percent: packet.gpu.load_percent,
            gpu_vram_used_mb: packet.gpu.vram_used_mb,
            gpu_vram_total_mb: packet.gpu.vram_total_mb,
            gpu_temperature_celsius: packet.gpu.temperature_celsius,
            
            memory_used_bytes: packet.memory.used_bytes,
            memory_total_bytes: packet.memory.total_bytes,
            memory_swap_used_pct: packet.memory.swap_used_pct,
            
            network_rx_bytes: packet.network.rx_bytes,
            network_tx_bytes: packet.network.tx_bytes,
            
            disk_used_bytes: packet.disk.used_bytes,
            disk_total_bytes: packet.disk.total_bytes,
            disk_read_bytes: packet.disk.read_bytes,
            disk_write_bytes: packet.disk.write_bytes,
            disk_partitions: packet.disk.partitions.clone(),
            
            uptime_seconds: packet.uptime_seconds,
            hmac_tag: packet.hmac_tag,
        }
    }

    /// Convert to nested [`MetricPacket`] for data model usage.
    pub fn to_nested(&self) -> crate::packet::MetricPacket {
        crate::packet::MetricPacket {
            version: self.version,
            machine_id: self.machine_id,
            timestamp: self.timestamp,
            sequence: self.sequence,
            
            cpu: crate::packet::CpuMetrics {
                usage_percent: self.cpu_usage_percent,
                temperature_celsius: self.cpu_temperature_celsius,
            },
            gpu: crate::packet::GpuMetrics {
                load_percent: self.gpu_load_percent,
                vram_used_mb: self.gpu_vram_used_mb,
                vram_total_mb: self.gpu_vram_total_mb,
                temperature_celsius: self.gpu_temperature_celsius,
            },
            memory: crate::packet::MemoryMetrics {
                used_bytes: self.memory_used_bytes,
                total_bytes: self.memory_total_bytes,
                swap_used_pct: self.memory_swap_used_pct,
            },
            network: crate::packet::NetworkMetrics {
                rx_bytes: self.network_rx_bytes,
                tx_bytes: self.network_tx_bytes,
            },
            disk: crate::packet::DiskMetrics {
                used_bytes: self.disk_used_bytes,
                total_bytes: self.disk_total_bytes,
                read_bytes: self.disk_read_bytes,
                write_bytes: self.disk_write_bytes,
                partitions: self.disk_partitions.clone(),
            },
            
            uptime_seconds: self.uptime_seconds,
            hmac_tag: self.hmac_tag,
        }
    }
}
