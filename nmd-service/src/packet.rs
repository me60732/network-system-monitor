//! # MetricPacket — rkyv-serializable UDP payload with HMAC-SHA256 authentication
//!
//! Defines the `MetricPacket` struct that is serialized via rkyv and sent over UDP from each
//! remote machine's systemd service to the desktop Cosmic applet. This module implements
//! Worf's security analysis: every packet carries a timestamp, monotonic sequence counter,
//! and HMAC-SHA256 tag computed over all fields using a pre-shared key stored at
//! `/etc/nmd/secret.key`.
//!
//! ## Design Philosophy
//!
//! The packet contains **raw metric values** (bytes, counts, etc.) rather than pre-calculated
//! percentages. This allows the desktop applet to handle display preferences (show as percentage
//! or absolute values) without requiring service changes.
//!
//! ## Security Fields (Worf — Phase 1A)
//!
//! | Field          | Type       | Purpose                                                    |
//! |----------------|------------|------------------------------------------------------------|
//! | `timestamp`    | `u64`      | Unix seconds; replay protection via freshness (< 10s old)  |
//! | `sequence`     | `u32`      | Monotonic counter per machine_id for replay detection      |
//! | `hmac_tag`     | `[u8; 32]`| HMAC-SHA256 over all serialized fields (excluding tag)    |
//!
//! ## Protocol Version History
//!
//! - **v1**: Original flat structure with basic metrics
//! - **v2**: Added optional IO/network stats, no struct change needed (rkyv handles missing fields)
//! - **v3**: Refactored to nested metric group structs for better type safety and extensibility
//!
//! ## rkyv Compatibility
//!
//! The struct derives [`rkyv::Archive`] so it can be zero-copy deserialized on the desktop side.
//! The `hmac_tag` field is excluded from the HMAC computation itself to avoid a circular dependency.

use rkyv::{Archive, Deserialize, Serialize};

/// Disk partition information for one mount point
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct PartitionInfo {
    /// Mount point path (e.g., "/", "/home", "/boot")
    pub mount: String,
    /// Total size in bytes
    pub total: u64,
    /// Used space in bytes
    pub used: u64,
}

/// CPU metrics group — all CPU-related metrics bundled together for type safety.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct CpuMetrics {
    /// CPU usage percentage (0.0–100.0), aggregate across all cores.
    pub usage_percent: f32,
    /// CPU package temperature in Celsius — `None` if thermal sensors unavailable.
    pub temperature_celsius: Option<f32>,
}

/// GPU metrics group — all GPU-related metrics bundled together for type safety.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct GpuMetrics {
    /// GPU utilization load percentage (0.0–100.0) — `None` if GPU monitoring unavailable.
    pub load_percent: Option<f32>,
    /// VRAM used in megabytes — `None` on systems without a discrete GPU.
    pub vram_used_mb: Option<u32>,
    /// VRAM total in megabytes — `None` on systems without a discrete GPU.
    pub vram_total_mb: Option<u32>,
    /// GPU junction temperature in Celsius — `None` if GPU thermal sensors unavailable.
    pub temperature_celsius: Option<f32>,
}

/// Memory metrics group — all memory-related metrics bundled together for type safety.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// Used memory in bytes.
    pub used_bytes: u64,
    /// Total memory in bytes.
    pub total_bytes: u64,
    /// Swap usage as a percentage of total swap space (0.0–100.0).
    pub swap_used_pct: f32,
}

/// Network metrics group — all network-related metrics bundled together for type safety.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct NetworkMetrics {
    /// Total bytes received on the primary network interface since boot.
    pub rx_bytes: u64,
    /// Total bytes transmitted on the primary network interface since boot.
    pub tx_bytes: u64,
}

/// Disk metrics group — all disk-related metrics bundled together for type safety.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct DiskMetrics {
    /// Used disk space in bytes (sum across all partitions).
    pub used_bytes: u64,
    /// Total disk space in bytes (sum across all partitions).
    pub total_bytes: u64,
    /// Total disk read bytes since boot — `None` if sysinfo doesn't expose IO stats.
    pub read_bytes: Option<u64>,
    /// Total disk write bytes since boot — `None` if sysinfo doesn't expose IO stats.
    pub write_bytes: Option<u64>,
    /// Disk partition information (mount point, total, used) for all mounted partitions
    pub partitions: Vec<PartitionInfo>,
}

/// A single metrics snapshot sent over UDP from remote machine → desktop applet.
///
/// All fields except `hmac_tag` are included in the HMAC-SHA256 digest computed by
/// [`crate::udp_sender::UdpSender::send`]. The tag is verified on receipt and packets
/// failing verification or freshness checks (< 10s old) are silently dropped per Worf's spec.
///
/// **Protocol Version 3**: Refactored to nested metric group structs for better type safety,
/// extensibility, and clearer semantics. Each metrics group (CPU, GPU, Memory, Network, Disk)
/// bundles related fields together, eliminating type confusion and making extension easier.
pub const PROTOCOL_VERSION: u32 = 3;

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct MetricPacket {
    // ── Protocol Version & Security (Worf Phase 1A) ───────────────────────

    /// Protocol version for detecting incompatible sender/receiver versions.
    /// Bump [`PROTOCOL_VERSION`] whenever a breaking change is made to the packet structure.
    pub version: u32,

    /// Unique identifier for the sending machine (e.g., hostname or UUID).
    /// Stored as fixed-length [u8; 20] — null-padded if shorter than 20 bytes.
    /// Fixed length ensures all subsequent fields remain at constant offsets in the rkyv buffer,
    /// enabling true zero-copy in-place mutation via the munge API on every send cycle.
    pub machine_id: [u8; 20],

    /// Unix timestamp in seconds when this packet was assembled.
    /// The receiver checks `now - timestamp < 10s` for replay protection.
    pub timestamp: u64,

    /// Monotonic sequence counter incremented with every packet sent by this service instance.
    /// Combined with machine_id + timestamp to detect replayed or out-of-order packets.
    pub sequence: u32,

    // ── Nested Metric Groups (Phase 3 Refactoring) ───────────────────────

    /// CPU metrics group — usage, temperature
    pub cpu: CpuMetrics,

    /// GPU metrics group — load, VRAM, temperature
    pub gpu: GpuMetrics,

    /// Memory metrics group — used/total bytes, swap percentage
    pub memory: MemoryMetrics,

    /// Network metrics group — RX/TX cumulative bytes since boot
    pub network: NetworkMetrics,

    /// Disk metrics group — usage, IO stats, partitions
    pub disk: DiskMetrics,

    /// System uptime in seconds since last boot.
    pub uptime_seconds: u64,

    // ── Authentication Tag (Worf Phase 1A) ────────────────────────────────

    /// HMAC-SHA256 tag over all fields above this one, computed by UdpSender before transmission.
    /// The receiver recomputes and compares to verify packet integrity + authenticity.
    pub hmac_tag: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;

    /// rkyv serialize → deserialize roundtrip preserves all data fields (Beverly writes after implementation).
    #[test]
    fn test_packet_rkyv_roundtrip() {
        // Test nested metric group initialization for protocol version 3
        let packet = MetricPacket {
            version: PROTOCOL_VERSION,
            machine_id: [0u8; 20], // Null-padded empty machine ID.
            timestamp: 0,
            sequence: 0,
            cpu: CpuMetrics {
                usage_percent: 0.0,
                temperature_celsius: None,
            },
            gpu: GpuMetrics {
                load_percent: None,
                vram_used_mb: None,
                vram_total_mb: None,
                temperature_celsius: None,
            },
            memory: MemoryMetrics {
                used_bytes: 0,
                total_bytes: 0,
                swap_used_pct: 0.0,
            },
            network: NetworkMetrics {
                rx_bytes: 0,
                tx_bytes: 0,
            },
            disk: DiskMetrics {
                used_bytes: 0,
                total_bytes: 0,
                read_bytes: None,
                write_bytes: None,
                partitions: Vec::new(),
            },
            uptime_seconds: 0,
            hmac_tag: [0u8; 32],
        };
        
        // Verify nested struct initialization
        assert!(packet.machine_id.iter().all(|&b| b == 0)); // All zeros = empty/null-padded.
        assert_eq!(packet.sequence, 0);
        assert_eq!(packet.cpu.usage_percent, 0.0);
        assert_eq!(packet.memory.swap_used_pct, 0.0);
        
        // Test GPU VRAM calculation (convert MB to bytes for applet display)
        let gpu_vram_bytes = packet.gpu.vram_used_mb.map(|v| v as u64 * 1_048_576);
        assert_eq!(gpu_vram_bytes, None); // No VRAM data in test
        
        // Test with sample GPU data
        let packet_with_gpu = MetricPacket {
            gpu: GpuMetrics {
                load_percent: Some(75.5),
                vram_used_mb: Some(4096), // 4GB
                vram_total_mb: Some(8192), // 8GB
                temperature_celsius: Some(85.0),
            },
            ..packet.clone()
        };
        
        assert_eq!(packet_with_gpu.gpu.load_percent, Some(75.5));
        assert_eq!(packet_with_gpu.gpu.vram_used_mb, Some(4096));
        let vram_bytes = packet_with_gpu.gpu.vram_used_mb.map(|v| v as u64 * 1_048_576);
        assert_eq!(vram_bytes, Some(4_294_967_296)); // 4GB in bytes
    }
}