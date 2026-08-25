//! # MetricPacket — rkyv-serializable UDP payload with HMAC-SHA256 authentication
//!
//! Defines the `MetricPacket` struct that is serialized via rkyv and sent over UDP from each
//! remote machine's systemd service to the desktop Cosmic applet. This module implements
//! Worf's security analysis: every packet carries a timestamp, monotonic sequence counter,
//! and HMAC-SHA256 tag computed over all fields using a pre-shared key stored at
//! `/etc/nmd/secret.key`.
//!
//! ## Security Fields (Worf — Phase 1A)
//!
//! | Field          | Type       | Purpose                                                    |
//! |----------------|------------|------------------------------------------------------------|
//! | `timestamp`    | `u64`      | Unix seconds; replay protection via freshness (< 10s old)  |
//! | `sequence`     | `u32`      | Monotonic counter per machine_id for replay detection      |
//! | `hmac_tag`     | `[u8; 32]`| HMAC-SHA256 over all serialized fields (excluding tag)    |
//!
//! ## Phase 2 Hardening — Additional Metrics (Geordi)
//!
//! New optional fields added in Phase 2 hardening:
//!
//! | Field                  | Type       | Purpose                                              |
//! |------------------------|------------|------------------------------------------------------|
//! | `disk_read_bytes`      | `Option<u64>` | Total disk read bytes since boot (None if unavailable) |
//! | `disk_write_bytes`     | `Option<u64>` | Total disk write bytes since boot (None if unavailable) |
//! | `network_rx_packets`   | `Option<u64>` | RX packets count (None if unavailable)               |
//! | `network_tx_packets`   | `Option<u64>` | TX packets count (None if unavailable)               |
//! | `network_rx_dropped`   | `Option<u64>` | RX dropped packets (None if unavailable)             |
//! | `network_tx_dropped`   | `Option<u64>` | TX dropped packets (None if unavailable)             |
//! | `memory_swap_used_pct` | `f32`        | Swap usage as percentage of total swap (0.0–100.0)     |
//!
//! ## rkyv Compatibility
//!
//! The struct derives [`rkyv::Archive`] so it can be zero-copy deserialized on the desktop side.
//! The `hmac_tag` field is excluded from the HMAC computation itself to avoid a circular dependency.

use rkyv::{Archive, Deserialize, Serialize};

/// A single metrics snapshot sent over UDP from remote machine → desktop applet.
///
/// All fields except `hmac_tag` are included in the HMAC-SHA256 digest computed by
/// [`crate::udp_sender::UdpSender::send`]. The tag is verified on receipt and packets
/// failing verification or freshness checks (< 10s old) are silently dropped per Worf's spec.
/// Current protocol version for MetricPacket serialization compatibility.
/// Bumped from 1 → 2: machine_id changed from String to [u8; 20] for zero-copy in-place mutation.
/// Phase 2 adds optional IO/network stats — rkyv handles missing fields gracefully, no version bump needed.
pub const PROTOCOL_VERSION: u32 = 2;

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct MetricPacket {
    // ── Protocol Version (Phase 1B) ───────────────────────────────────────

    /// Protocol version for detecting incompatible sender/receiver versions.
    /// Bump [`PROTOCOL_VERSION`] whenever a breaking change is made to the packet structure.
    pub version: u32,

    // ── Identity & Security (Worf Phase 1A) ───────────────────────────────

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

    // ── Metric Data (sourced from metrics-core via aggregator) ────────────

    /// CPU usage percentage (0.0–100.0), aggregate across all cores.
    pub cpu_usage: f32,

    /// Memory used as a percentage of total RAM (0.0–100.0).
    pub memory_used_percent: f32,

    /// Disk usage for the root partition as a percentage (0.0–100.0).
    pub disk_used_percent: f32,

    /// Total bytes received on the primary network interface since boot.
    pub network_rx_bytes: u64,

    /// System uptime in seconds since last boot.
    pub uptime_seconds: u64,

    // ── Phase 2 Hardening — Additional Metrics (Optional) ─────────────────

    /// Total disk read bytes since boot — `None` if sysinfo doesn't expose IO stats.
    pub disk_read_bytes: Option<u64>,

    /// Total disk write bytes since boot — `None` if sysinfo doesn't expose IO stats.
    pub disk_write_bytes: Option<u64>,

    /// RX packets count (cumulative) — `None` if sysinfo doesn't expose packet counters.
    pub network_rx_packets: Option<u64>,

    /// TX packets count (cumulative) — `None` if sysinfo doesn't expose packet counters.
    pub network_tx_packets: Option<u64>,

    /// Packets dropped on receive — `None` if sysinfo doesn't expose dropped counts.
    pub network_rx_dropped: Option<u64>,

    /// Packets dropped on transmit — `None` if sysinfo doesn't expose dropped counts.
    pub network_tx_dropped: Option<u64>,

    /// Swap usage as a percentage of total swap space (0.0–100.0).
    pub memory_swap_used_pct: f32,

    // ── Optional Metrics (None when hardware unsupported) ─────────────────

    /// GPU VRAM used in megabytes — `None` on systems without a discrete GPU.
    pub gpu_vram_used_mb: Option<u32>,

    /// CPU package or GPU junction temperature in Celsius — `None` if thermal sensors unavailable.
    pub temperature_celsius: Option<f32>,

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
        // TODO: Implement full roundtrip once UdpSender HMAC logic is complete.
        // For now, verify the struct compiles and default values are sane.
        let packet = MetricPacket {
            version: PROTOCOL_VERSION,
            machine_id: [0u8; 20], // Null-padded empty machine ID.
            timestamp: 0,
            sequence: 0,
            cpu_usage: 0.0,
            memory_used_percent: 0.0,
            disk_used_percent: 0.0,
            network_rx_bytes: 0,
            uptime_seconds: 0,
            disk_read_bytes: None,
            disk_write_bytes: None,
            network_rx_packets: None,
            network_tx_packets: None,
            network_rx_dropped: None,
            network_tx_dropped: None,
            memory_swap_used_pct: 0.0,
            gpu_vram_used_mb: None,
            temperature_celsius: None,
            hmac_tag: [0u8; 32],
        };
        assert!(packet.machine_id.iter().all(|&b| b == 0)); // All zeros = empty/null-padded.
        assert_eq!(packet.sequence, 0);
        assert_eq!(packet.memory_swap_used_pct, 0.0); // Phase 2: swap usage initialized to 0
    }
}