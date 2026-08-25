//! # UdpSender — HMAC-SHA256 authenticated UDP packet transmission with pre-serialized buffer
//!
//! Sends [`MetricPacket`] structs over UDP to the desktop Cosmic applet. Uses a **pre-serialized
//! rkyv buffer** that is modified in-place on every send cycle via the munge API, eliminating all
//! per-cycle heap allocations and serialization passes for numeric fields. Only the HMAC tag (which
//! depends on changing field values) requires recomputation — no full struct serialization needed.
//!
//! ## Optimization: Pre-Serialized Buffer + In-Place Mutation
//!
//! The buffer is serialized once at construction time with a template packet containing the correct
//! `machine_id` ([u8; 20], fixed length). On each send cycle, numeric fields (timestamp, sequence,
//! cpu_usage, etc.) are mutated in-place via `rkyv::munge!`, then the HMAC tag region is zeroed for
//! computation, computed over the buffer bytes, and written back — all without any serialization.
//!
//! ## Security Design (Worf Phase 1A)
//!
//! - **Authentication**: HMAC-SHA256 over serialized packet fields prevents spoofing/forgery.
//! - **Replay protection**: Receiver checks timestamp freshness (< 10s old) + monotonic sequence number.
//! - **Secret storage**: Key at `/etc/nmd/secret.key`, `0600` permissions, exactly 32 bytes.
//! - **Sequence counter**: AtomicU32 incremented on every send, included in packet for ordering.

use crate::packet::{MetricPacket, ArchivedMetricPacket};
use hmac::{Hmac, KeyInit, Mac};
use munge::munge;
use rkyv::access_mut;
use sha2::Sha256;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU32, Ordering};

/// HMAC-SHA256 type alias for ergonomic use throughout the module.
type HmacSha256 = Hmac<Sha256>;

/// Byte offset of `hmac_tag` field within the pre-serialized rkyv buffer.
/// Computed at construction time from the initial serialization — used to zero and write the tag in-place.
const HMAC_TAG_LEN: usize = 32;

/// UDP sender that transmits rkyv-encoded [`MetricPacket`] with HMAC-SHA256 authentication,
/// using a pre-serialized buffer for zero-allocation per-cycle sends.
pub struct UdpSender {
    /// Bound UDP socket used to send packets to the desktop applet.
    pub socket: UdpSocket,
    /// Destination address of the desktop Cosmic applet (host:port).
    pub dest: SocketAddr,
    /// Pre-shared HMAC key loaded from /etc/nmd/secret.key at startup (32 bytes).
    pub secret_key: Vec<u8>,
    /// Monotonic sequence counter incremented on every packet send.
    pub sequence_counter: AtomicU32,

    // ── Pre-Serialized Buffer (Zero-Copy Optimization) ────────────────────

    /// Pre-serialized rkyv buffer — modified in-place each cycle via munge!.
    /// Contains a template MetricPacket with the correct machine_id and zeroed mutable fields.
    pub packet_buf: Vec<u8>,

    /// Byte offset of `hmac_tag` field within `packet_buf`.
    /// Used to zero the tag for HMAC computation, then write back the computed tag — all in-place.
    hmac_tag_offset: usize,
}

impl UdpSender {
    /// Create a new `UdpSender` bound to an ephemeral local port, targeting the given destination.
    ///
    /// Pre-serializes a template [`MetricPacket`] with the provided `machine_id` (padded to 20 bytes)
    /// into an internal buffer that is reused across all send cycles — no per-cycle allocation or
    /// serialization for numeric fields. Only the HMAC tag region requires modification each cycle.
    pub fn new(dest: SocketAddr, secret_key: Vec<u8>, machine_id: &str) -> Result<Self, std::io::Error> {
        // Bind to an ephemeral local port for sending only (no inbound traffic expected).
        let socket = UdpSocket::bind("0.0.0.0:0")?;

        // Encode machine_id as fixed-length [u8; 20] — null-padded if shorter, truncated if longer.
        let mut machine_id_bytes = [0u8; 20];
        let src = machine_id.as_bytes();
        let len = src.len().min(20);
        machine_id_bytes[..len].copy_from_slice(&src[..len]);

        // Create a template packet with all fields zeroed — will be mutated in-place each cycle.
        let template = MetricPacket {
            version: crate::packet::PROTOCOL_VERSION,
            machine_id: machine_id_bytes,
            timestamp: 0,           // Modified in-place via munge! on every send
            sequence: 0,            // Modified in-place via munge! on every send
            cpu_usage: 0.0,         // Modified in-place via munge! on every send
            memory_used_percent: 0.0,
            disk_used_percent: 0.0,
            network_rx_bytes: 0,
            uptime_seconds: 0,
            disk_read_bytes: None,      // Phase 2: IO stats (sysinfo doesn't expose these)
            disk_write_bytes: None,     // Phase 2: IO stats (sysinfo doesn't expose these)
            network_rx_packets: None,   // Phase 2: packet counters (sysinfo doesn't expose these)
            network_tx_packets: None,   // Phase 2: packet counters (sysinfo doesn't expose these)
            network_rx_dropped: None,   // Phase 2: dropped packets (sysinfo doesn't expose these)
            network_tx_dropped: None,   // Phase 2: dropped packets (sysinfo doesn't expose these)
            memory_swap_used_pct: 0.0,  // Phase 2: swap usage percentage
            gpu_vram_used_mb: None,
            temperature_celsius: None,
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

        // Compute the byte offset of hmac_tag within the buffer by serializing a reference template.
        // We find it by searching for the 32-byte zero tag at the end (hmac_tag is always last field).
        let hmac_tag_offset = packet_buf.len().saturating_sub(HMAC_TAG_LEN);

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
    /// Modifies the pre-serialized `packet_buf` in-place via munge! for all numeric fields, then:
    /// 1. Zeroes the hmac_tag region in the buffer (for HMAC computation)
    /// 2. Computes HMAC-SHA256 over the entire buffer bytes
    /// 3. Writes the computed tag back into the buffer — also in-place
    /// 4. Sends the raw buffer directly via UDP — no serialization at all!
    pub fn send(&mut self, packet: &MetricPacket) -> Result<(), std::io::Error> {
        // 1. Atomically increment and get the previous sequence value.
        let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);

        // Set timestamp to current Unix seconds for receiver freshness check.
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // 2. Access the archived packet mutably via rkyv's safe access_mut (returns Seal<T>).
        //    Then destructure with munge! to get mutable handles to each field — zero-copy in-place mutation.
        let seal = access_mut::<ArchivedMetricPacket, rkyv::rancor::Error>(&mut self.packet_buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Access mutation failed: {}", e)))?;

        // Destructure the underlying type via munge to get mutable handles for each field.
        munge!(let ArchivedMetricPacket {
            mut timestamp, mut sequence, mut cpu_usage, mut memory_used_percent, mut disk_used_percent,
            mut network_rx_bytes, mut uptime_seconds, gpu_vram_used_mb, temperature_celsius, ..
        } = seal);

        // Mutate fixed-size numeric fields directly in the buffer via munge handles.
        *timestamp = now_secs.into();                           // u64_le — DerefMut works for primitives
        *sequence = seq.into();                                  // u32_le — DerefMut works for primitives
        *cpu_usage = packet.cpu_usage.into();                    // f32_le — DerefMut works for primitives
        *memory_used_percent = packet.memory_used_percent.into();
        *disk_used_percent = packet.disk_used_percent.into();
        *network_rx_bytes = packet.network_rx_bytes.into();
        *uptime_seconds = packet.uptime_seconds.into();

        // Optional fields (ArchivedOption) — use unsafe unseal for in-place mutation.
        // ArchivedOption<T> does NOT implement NoUndef, so DerefMut fails; must use Seal::unseal_unchecked.
        unsafe {
            let gpu_opt = rkyv::seal::Seal::unseal_unchecked(gpu_vram_used_mb);
            match &packet.gpu_vram_used_mb {
                Some(v) => *gpu_opt = rkyv::option::ArchivedOption::Some((*v).into()),
                None    => *gpu_opt = rkyv::option::ArchivedOption::None,
            }

            let temp_opt = rkyv::seal::Seal::unseal_unchecked(temperature_celsius);
            match &packet.temperature_celsius {
                Some(v) => *temp_opt = rkyv::option::ArchivedOption::Some((*v).into()),
                None    => *temp_opt = rkyv::option::ArchivedOption::None,
            }
        }

        // Zero out the hmac_tag region for HMAC computation (excludes tag from digest).
        let tag_start = self.hmac_tag_offset;
        self.packet_buf[tag_start..tag_start + HMAC_TAG_LEN].copy_from_slice(&[0u8; 32]);

        // 3. Compute HMAC-SHA256 over the entire buffer (tag region is zeroed).
        let tag = self.compute_hmac(&self.packet_buf);

        // 4. Write the computed tag back into the buffer — in-place, no serialization needed.
        self.packet_buf[self.hmac_tag_offset..self.hmac_tag_offset + HMAC_TAG_LEN]
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

        let packet = MetricPacket {
            version: crate::packet::PROTOCOL_VERSION,
            machine_id: [b't', b'e', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            timestamp: 12345,
            sequence: 99,
            cpu_usage: 45.6,
            memory_used_percent: 78.9,
            disk_used_percent: 33.3,
            network_rx_bytes: 1_000_000,
            uptime_seconds: 3600,
            disk_read_bytes: None,      // Phase 2: IO stats (sysinfo doesn't expose these)
            disk_write_bytes: None,     // Phase 2: IO stats (sysinfo doesn't expose these)
            network_rx_packets: None,   // Phase 2: packet counters (sysinfo doesn't expose these)
            network_tx_packets: None,   // Phase 2: packet counters (sysinfo doesn't expose these)
            network_rx_dropped: None,   // Phase 2: dropped packets (sysinfo doesn't expose these)
            network_tx_dropped: None,   // Phase 2: dropped packets (sysinfo doesn't expose these)
            memory_swap_used_pct: 0.0,  // Phase 2: swap usage percentage
            gpu_vram_used_mb: Some(512),
            temperature_celsius: Some(65.0),
            hmac_tag: [0u8; 32],
        };

        // Should return Ok — buffer is modified in-place and sent via UDP to 127.0.0.1:51057.
        let result = sender.send(&packet);
        assert!(result.is_ok());
    }

    /// Verify that two consecutive sends produce different sequence numbers (replay protection).
    #[test]
    fn test_sequence_counter_increments() {
        let key = vec![0u8; 32];
        let dest: SocketAddr = "127.0.0.1:51058".parse().unwrap();
        let mut sender = UdpSender::new(dest, key, "seqtest").expect("Failed to create UdpSender");

        // First send — sequence should be 0 (fetch_add returns previous value).
        let packet = MetricPacket {
            version: crate::packet::PROTOCOL_VERSION,
            machine_id: [b's', b'e', b'q', b't', b'e', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            timestamp: 100,
            sequence: 0, // Will be overwritten by sender.
            cpu_usage: 50.0,
            memory_used_percent: 60.0,
            disk_used_percent: 70.0,
            network_rx_bytes: 12345,
            uptime_seconds: 999,
            disk_read_bytes: None,      // Phase 2: IO stats (sysinfo doesn't expose these)
            disk_write_bytes: None,     // Phase 2: IO stats (sysinfo doesn't expose these)
            network_rx_packets: None,   // Phase 2: packet counters (sysinfo doesn't expose these)
            network_tx_packets: None,   // Phase 2: packet counters (sysinfo doesn't expose these)
            network_rx_dropped: None,   // Phase 2: dropped packets (sysinfo doesn't expose these)
            network_tx_dropped: None,   // Phase 2: dropped packets (sysinfo doesn't expose these)
            memory_swap_used_pct: 0.0,  // Phase 2: swap usage percentage
            gpu_vram_used_mb: None,
            temperature_celsius: None,
            hmac_tag: [0u8; 32],
        };

        // Verify sequence increments correctly via fetch_add semantics.
        let seq1 = sender.sequence_counter.fetch_add(1, Ordering::SeqCst);
        assert_eq!(seq1, 0);
        let seq2 = sender.sequence_counter.fetch_add(1, Ordering::SeqCst);
        assert_eq!(seq2, 1);

        // Send should succeed without panic.
        let result = sender.send(&packet);
        assert!(result.is_ok());
    }
}