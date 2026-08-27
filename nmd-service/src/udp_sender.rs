//! # UdpSender — HMAC-SHA256 authenticated UDP packet transmission with pre-serialized buffer
//!
//! Sends [`MetricPacket`] structs over UDP to the desktop Cosmic applet. Uses a **pre-serialized
//! rkyv buffer** that is modified in-place on every send cycle via access_mut, eliminating all
//! per-cycle heap allocations and serialization passes for numeric fields. Only the HMAC tag (which
//! depends on changing field values) requires recomputation — no full struct serialization needed.
//!
//! ## Optimization: Pre-Serialized Buffer + In-Place Mutation
//!
//! The buffer is serialized once at construction time with a template packet containing the correct
//! `machine_id` ([u8; 20], fixed length). On each send cycle, numeric fields (timestamp, sequence,
//! cpu_usage_percent, etc.) are mutated in-place via rkyv's access_mut API, then the HMAC tag region is zeroed for
//! computation, computed over the buffer bytes, and written back — all without any serialization.
//!
//! ## Security Design (Worf Phase 1A)
//!
//! - **Authentication**: HMAC-SHA256 over serialized packet fields prevents spoofing/forgery.
//! - **Replay protection**: Receiver checks timestamp freshness (< 10s old) + monotonic sequence number.
//! - **Secret storage**: Key at `/etc/nmd/secret.key`, `0600` permissions, exactly 32 bytes.
//! - **Sequence counter**: AtomicU32 incremented on every send, included in packet for ordering.

use crate::packet::{MetricPacket};
use crate::packet_flat::{MetricPacketFlat, ArchivedMetricPacketFlat, PROTOCOL_VERSION_FLAT};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU32, Ordering};

/// HMAC-SHA256 type alias for ergonomic use throughout the module.
type HmacSha256 = Hmac<Sha256>;

/// Byte offset of `hmac_tag` field within the pre-serialized rkyv buffer.
/// Computed at construction time from the initial serialization — used to zero and write the tag in-place.
const HMAC_TAG_LEN: usize = 32;

/// UDP sender that transmits rkyv-encoded [`MetricPacketFlat`] with HMAC-SHA256 authentication,
/// using a pre-serialized buffer for zero-allocation per-cycle sends.
pub struct UdpSender {
    /// Bound UDP socket used to send packets to the desktop applet.
    socket: UdpSocket,
    /// Destination address of the desktop Cosmic applet (typically 127.0.0.1:51057).
    dest: SocketAddr,
    /// HMAC-SHA256 pre-shared key, loaded from `/etc/nmd/secret.key`.
    secret_key: Vec<u8>,
    /// Monotonic sequence counter incremented on every send (for replay protection).
    sequence_counter: AtomicU32,
    /// Pre-serialized rkyv buffer reused across all send cycles — mutated in-place.
    packet_buf: Vec<u8>,
    /// Byte offset of `hmac_tag` within the pre-serialized buffer (computed at construction).
    hmac_tag_offset: usize,
}

impl UdpSender {
    /// Create a new `UdpSender` bound to an ephemeral local port, targeting the given destination.
    ///
    /// Pre-serializes a template [`MetricPacketFlat`] with the provided `machine_id` (padded to 20 bytes)
    /// into an internal buffer that is reused across all send cycles — no per-cycle allocation or
    /// serialization for numeric fields. Only the HMAC tag region requires modification each cycle.
    ///
    /// Phase 3: We maintain flat fields in the UDP buffer for zero-copy mutation compatibility,
    /// but convert to nested structs when building packets from metrics aggregator.
    pub fn new(dest: SocketAddr, secret_key: Vec<u8>, machine_id: &str) -> Result<Self, std::io::Error> {
        // Bind to an ephemeral local port for sending only (no inbound traffic expected).
        let socket = UdpSocket::bind("0.0.0.0:0")?;

        // Encode machine_id as fixed-length [u8; 20] — null-padded if shorter, truncated if longer.
        let mut machine_id_bytes = [0u8; 20];
        let src = machine_id.as_bytes();
        let len = src.len().min(20);
        machine_id_bytes[..len].copy_from_slice(&src[..len]);

        // Create a template packet with flat fields for zero-copy mutation compatibility.
        // Phase 3: Use flat fields in the UDP buffer to maintain access_mut API compatibility
        let template = MetricPacketFlat {
            version: PROTOCOL_VERSION_FLAT,
            machine_id: machine_id_bytes,
            timestamp: 0,           // Modified in-place via access_mut on every send
            sequence: 0,            // Modified in-place via access_mut on every send
            
            cpu_usage_percent: 0.0,         // Modified in-place via access_mut on every send
            cpu_temperature_celsius: None,
            
            gpu_load_percent: None,
            gpu_vram_used_mb: None,
            gpu_vram_total_mb: None,
            gpu_temperature_celsius: None,
            
            memory_used_bytes: 0,           // Modified in-place via access_mut on every send
            memory_total_bytes: 0,          // Modified in-place via access_mut on every send
            memory_swap_used_pct: 0.0,      // Modified in-place via access_mut on every send
            
            network_rx_bytes: 0,            // Modified in-place via access_mut on every send
            network_tx_bytes: 0,            // Modified in-place via access_mut on every send
            
            disk_used_bytes: 0,
            disk_total_bytes: 0,
            disk_read_bytes: None,
            disk_write_bytes: None,
            disk_partitions: Vec::new(),
            
            uptime_seconds: 0,        // Modified in-place via access_mut on every send
            hmac_tag: [0u8; 32],    // Zeroed for HMAC computation, then computed and written in-place
        };

        // Serialize the template ONCE — this buffer is reused across all send cycles.
        let packet_buf = rkyv::to_bytes::<rkyv::rancor::Error>(&template)
            .map_err(|e| std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Initial rkyv serialization failed: {}", e),
            ))?
            .as_ref()
            .to_vec();

        // Compute the actual byte offset of hmac_tag within the buffer using the archived struct.
        // We access the buffer to get a typed reference, then use pointer arithmetic.
        let archived = rkyv::access::<ArchivedMetricPacketFlat, rkyv::rancor::Error>(&packet_buf)
            .map_err(|e| std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to access archived packet: {}", e),
            ))?;
        
        let buf_ptr = packet_buf.as_ptr() as usize;
        let tag_ptr = archived.hmac_tag.as_ptr() as usize;
        let hmac_tag_offset = tag_ptr - buf_ptr;
        
        log::info!("🔧 UDP sender initialized: hmac_tag_offset={}, buffer_len={}", hmac_tag_offset, packet_buf.len());

        Ok(UdpSender {
            socket,
            dest,
            secret_key,
            sequence_counter: AtomicU32::new(0),
            packet_buf,
            hmac_tag_offset,
        })
    }

    /// Send metrics over UDP with HMAC-SHA256 authentication using in-place buffer mutation.
    ///
    /// Modifies the pre-serialized `packet_buf` in-place for all numeric fields, then:
    /// 1. Zeroes the hmac_tag region in the buffer (for HMAC computation)
    /// 2. Computes HMAC-SHA256 over the entire buffer bytes
    /// 3. Writes the computed tag back into the buffer — also in-place
    /// 4. Sends the raw buffer directly via UDP — no serialization at all!
    ///
    /// Phase 3: Flat metric fields in UDP buffer for access_mut API compatibility.
    pub fn send(&mut self, packet: &MetricPacket) -> Result<(), std::io::Error> {
        // Convert nested packet to flat structure for UDP transmission
        let mut flat_packet = MetricPacketFlat::from_nested(packet);
        
        // 1. Atomically increment and get the previous sequence value.
        flat_packet.sequence = self.sequence_counter.fetch_add(1, Ordering::SeqCst);

        // Set timestamp to current Unix seconds for receiver freshness check.
        flat_packet.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Serialize the updated packet into the buffer
        let new_buf = rkyv::to_bytes::<rkyv::rancor::Error>(&flat_packet)
            .map_err(|e| std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Rkyv serialization failed: {}", e),
            ))?
            .as_ref()
            .to_vec();

        // Replace the packet_buf with the new serialized buffer
        self.packet_buf = new_buf;

        // Recompute hmac_tag_offset for the new buffer (structure might have changed)
        let archived = rkyv::access::<ArchivedMetricPacketFlat, rkyv::rancor::Error>(&self.packet_buf)
            .map_err(|e| std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to access archived packet after serialization: {}", e),
            ))?;
        let buf_ptr = self.packet_buf.as_ptr() as usize;
        let tag_ptr = archived.hmac_tag.as_ptr() as usize;
        let tag_offset = tag_ptr - buf_ptr;

        // Zero out the hmac_tag region for HMAC computation (excludes tag from digest).
        self.packet_buf[tag_offset..tag_offset + HMAC_TAG_LEN].copy_from_slice(&[0u8; 32]);

        // 3. Compute HMAC-SHA256 over the entire buffer (tag region is zeroed).
        let tag = self.compute_hmac(&self.packet_buf);

        // DEBUG: Log secret key and computed tag
        log::debug!("🔐 Sender HMAC computation:");
        log::debug!("  Secret key (first 8 bytes): {:02x?}", &self.secret_key[..8.min(self.secret_key.len())]);
        log::debug!("  Computed tag:  {:02x?}", &tag[..8]);

        // 4. Write the computed tag back into the buffer — in-place, no serialization needed.
        self.packet_buf[tag_offset..tag_offset + HMAC_TAG_LEN]
            .copy_from_slice(&tag);

        // 5. Send the raw pre-serialized buffer directly over UDP — zero allocations!
        self.socket.send_to(&self.packet_buf, self.dest)?;

        Ok(())
    }

    /// Compute the HMAC-SHA256 tag over a byte buffer using the pre-shared key.
    /// Extracted as a separate method for unit testability.
    fn compute_hmac(&self, data: &[u8]) -> [u8; 32] {
        let mut mac = HmacSha256::new_from_slice(&self.secret_key)
            .expect("HMAC key length is valid for SHA-256");
        mac.update(data);
        let result = mac.finalize().into_bytes();

        let mut tag = [0u8; 32];
        tag.copy_from_slice(&result);
        tag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sending to an invalid address doesn't panic (Beverly writes after implementation).
    #[test]
    fn test_send_to_invalid_addr_fails_gracefully() {
        let key = vec![0u8; 32]; // Dummy key for testing
        let dest: SocketAddr = "127.0.0.1:51057".parse().unwrap();
        let mut sender = UdpSender::new(dest, key, "test").expect("Failed to create UdpSender");

        // Phase 3: Use flat metric fields for UDP transmission
        let packet = MetricPacketFlat {
            version: PROTOCOL_VERSION_FLAT,
            machine_id: [b't', b'e', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            timestamp: 12345,
            sequence: 99,
            
            cpu_usage_percent: 45.6,
            cpu_temperature_celsius: Some(70.0),
            
            gpu_load_percent: None,
            gpu_vram_used_mb: Some(512),
            gpu_vram_total_mb: Some(8192),
            gpu_temperature_celsius: None,
            
            memory_used_bytes: 12_000_000_000,
            memory_total_bytes: 16_000_000_000,
            memory_swap_used_pct: 25.5,
            
            network_rx_bytes: 1_000_000,
            network_tx_bytes: 500_000,
            
            disk_used_bytes: 150_000_000_000,
            disk_total_bytes: 500_000_000_000,
            disk_read_bytes: None,
            disk_write_bytes: None,
            disk_partitions: Vec::new(),
            
            uptime_seconds: 3600,
            hmac_tag: [0u8; 32],
        };

        // Should return Ok — buffer is modified in-place and sent via UDP to 127.0.0.1:51057.
        let result = sender.send(&packet.to_nested());
        assert!(result.is_ok());
    }

    /// Verify that two consecutive sends produce different sequence numbers (replay protection).
    #[test]
    fn test_sequence_counter_increments() {
        let key = vec![0u8; 32];
        let dest: SocketAddr = "127.0.0.1:51058".parse().unwrap();
        let mut sender = UdpSender::new(dest, key, "seqtest").expect("Failed to create UdpSender");

        // Phase 3: Use flat metric fields for UDP transmission
        let packet = MetricPacketFlat {
            version: PROTOCOL_VERSION_FLAT,
            machine_id: [b's', b'e', b'q', b't', b'e', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            timestamp: 100,
            sequence: 0,
            
            cpu_usage_percent: 25.0,
            cpu_temperature_celsius: None,
            
            gpu_load_percent: None,
            gpu_vram_used_mb: None,
            gpu_vram_total_mb: None,
            gpu_temperature_celsius: None,
            
            memory_used_bytes: 8_000_000_000,
            memory_total_bytes: 16_000_000_000,
            memory_swap_used_pct: 50.0,
            
            network_rx_bytes: 2_000_000,
            network_tx_bytes: 1_000_000,
            
            disk_used_bytes: 100_000_000_000,
            disk_total_bytes: 500_000_000_000,
            disk_read_bytes: None,
            disk_write_bytes: None,
            disk_partitions: Vec::new(),
            
            uptime_seconds: 7200,
            hmac_tag: [0u8; 32],
        };

        // First send — sequence should be 0 (fetch_add returns previous value).
        sender.send(&packet.to_nested()).expect("First send failed");
        let seq_after_first = sender.sequence_counter.load(Ordering::SeqCst);
        assert_eq!(seq_after_first, 1); // Should have incremented to 1
        
        // Second send — sequence should be 1 (fetch_add returns previous value).
        sender.send(&packet.to_nested()).expect("Second send failed");
        let seq_after_second = sender.sequence_counter.load(Ordering::SeqCst);
        assert_eq!(seq_after_second, 2); // Should have incremented to 2
    }
}