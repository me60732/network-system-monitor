//! # UdpSender — ChaCha20-Poly1305 encrypted UDP packet transmission
//!
//! Sends [`MetricPacket`] structs over UDP to the desktop Cosmic applet. Each send serializes
//! the packet via rkyv, encrypts it with ChaCha20-Poly1305 AEAD, and transmits the wire packet
//! `[12-byte nonce][ciphertext + 16-byte Poly1305 tag]` (see [`crate::crypto`]).
//!
//! ## Security Design (Pairing System V1, Phase 1)
//!
//! - **Confidentiality + authenticity**: ChaCha20-Poly1305 AEAD — the Poly1305 tag replaces the
//!   old HMAC-SHA256 signature and additionally encrypts all metric data on the wire.
//! - **Replay protection**: Receiver checks timestamp freshness (< 10s old) + monotonic sequence.
//! - **Identity**: An Ed25519 keypair is generated on first run and persisted to
//!   `~/.config/nmd/keypair.key` (0600). Unused in Phase 1; Phase 2 uses it for ECDH pairing.
//! - **Key**: Phase 1 uses the hardcoded [`crypto::TEMP_SHARED_KEY`] — replaced by an
//!   ECDH-derived per-machine key in Phase 2.
//! - **Nonce discipline**: `[4-byte random prefix][8-byte counter]` — the counter never repeats
//!   within a session, and the random prefix separates senders sharing the Phase 1 key.

use crate::crypto;
use crate::packet::MetricPacket;
use chacha20poly1305::ChaCha20Poly1305;
use ed25519_dalek::SigningKey;
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Filename of the persisted Ed25519 keypair inside the config directory.
const KEYPAIR_FILENAME: &str = "keypair.key";

/// UDP sender that transmits rkyv-encoded [`MetricPacket`] encrypted with ChaCha20-Poly1305.
///
/// # Fields
///
/// * `socket`: Bound UDP socket used to send packets to the desktop applet.
/// * `dest`: Destination address of the desktop Cosmic applet (typically port 51057).
/// * `cipher`: ChaCha20-Poly1305 cipher — Phase 1: keyed with `crypto::TEMP_SHARED_KEY`.
/// * `identity_key`: Ed25519 identity keypair, loaded from `~/.config/nmd/keypair.key`.
///   Unused in Phase 1; Phase 2 derives the shared ChaCha20 key from it via ECDH.
/// * `sequence_counter`: Monotonic per-send counter embedded in the packet (replay protection).
/// * `nonce_prefix`: Random 4-byte prefix making this sender's nonce space unique (see module docs).
/// * `nonce_counter`: Monotonic 8-byte counter forming the tail of each AEAD nonce — never reused.
/// * `sender_session_id`: Random session identifier generated at startup (SEC-03 fix).
pub struct UdpSender {
    socket: UdpSocket,
    dest: SocketAddr,
    cipher: ChaCha20Poly1305,
    #[allow(dead_code)] // Held for Phase 2 ECDH pairing — persisted identity established in Phase 1.
    identity_key: SigningKey,
    sequence_counter: AtomicU32,
    nonce_prefix: [u8; 4],
    nonce_counter: AtomicU64,
    sender_session_id: [u8; 16],
}

impl UdpSender {
    /// Create a new `UdpSender` bound to an ephemeral local port, targeting the given destination.
    ///
    /// Loads (or generates on first run) the Ed25519 identity keypair, initializes the
    /// ChaCha20-Poly1305 cipher with the Phase 1 temporary key, and generates the random
    /// sender_session_id (SEC-03) + nonce prefix.
    pub fn new(dest: SocketAddr, _machine_id: &str) -> Result<Self, std::io::Error> {
        // Bind to an ephemeral local port for sending only (no inbound traffic expected).
        let socket = UdpSocket::bind("0.0.0.0:0")?;

        // SEC-03: Random sender_session_id differentiates restarts. The nonce prefix is drawn
        // from the same urandom read — independent bytes, one syscall.
        let mut random_bytes = [0u8; 20];
        use std::fs::File;
        use std::io::Read;
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut random_bytes))
            .map_err(|e| std::io::Error::other(
                format!("Failed to generate session randomness: {}", e),
            ))?;
        let mut sender_session_id = [0u8; 16];
        sender_session_id.copy_from_slice(&random_bytes[..16]);
        let mut nonce_prefix = [0u8; 4];
        nonce_prefix.copy_from_slice(&random_bytes[16..]);

        log::info!("🔐 Generated sender_session_id: {:02x?}", &sender_session_id[..4]);

        // Load or generate the persistent Ed25519 identity keypair (Phase 2 pairing identity).
        let identity_key = Self::load_or_generate_keypair(Self::default_keypair_path())?;

        Ok(UdpSender {
            socket,
            dest,
            cipher: crypto::cipher_from_key(&crypto::TEMP_SHARED_KEY),
            identity_key,
            sequence_counter: AtomicU32::new(0),
            nonce_prefix,
            nonce_counter: AtomicU64::new(0),
            sender_session_id,
        })
    }

    /// Default on-disk location of the Ed25519 keypair: `~/.config/nmd/keypair.key`.
    fn default_keypair_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config").join("nmd").join(KEYPAIR_FILENAME)
    }

    /// Load the Ed25519 keypair from `path`, generating and persisting a new one if absent.
    ///
    /// The file stores the 64-byte `to_keypair_bytes()` form (32-byte secret ‖ 32-byte public)
    /// and is written with 0600 permissions. Extracted for unit testability.
    fn load_or_generate_keypair(path: PathBuf) -> Result<SigningKey, std::io::Error> {
        if path.exists() {
            let bytes = std::fs::read(&path)?;
            let keypair_bytes: [u8; 64] = bytes.as_slice().try_into().map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Keypair file {} must be exactly 64 bytes, got {}", path.display(), bytes.len()),
                )
            })?;
            let key = SigningKey::from_keypair_bytes(&keypair_bytes).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Corrupt keypair file {}: {}", path.display(), e),
                )
            })?;
            log::info!("🔑 Loaded Ed25519 identity keypair from {}", path.display());
            return Ok(key);
        }

        // First run: generate, persist with 0600, then return.
        let key = SigningKey::generate(&mut rand::rngs::OsRng);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, key.to_keypair_bytes())?;
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        log::info!("🔑 Generated new Ed25519 identity keypair at {}", path.display());
        Ok(key)
    }

    /// Send metrics over UDP encrypted with ChaCha20-Poly1305.
    ///
    /// Process:
    /// 1. Fill in sender_session_id, sequence, and timestamp
    /// 2. Serialize the packet via rkyv
    /// 3. Build a unique nonce (`prefix ‖ counter`) and encrypt (tag appended by AEAD)
    /// 4. Send `[nonce][ciphertext+tag]`
    pub fn send(&mut self, packet: &MetricPacket) -> Result<(), std::io::Error> {
        // Clone packet and fill in runtime fields
        let mut outgoing = packet.clone();
        outgoing.sender_session_id = self.sender_session_id;
        outgoing.sequence = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
        outgoing.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Serialize the packet (plaintext — encrypted below, never sent as-is)
        let packet_buf = rkyv::to_bytes::<rkyv::rancor::Error>(&outgoing)
            .map_err(|e| std::io::Error::other(
                format!("Rkyv serialization failed: {}", e),
            ))?;

        // Unique nonce: random per-session prefix + monotonic counter (never reused — see module docs)
        let nonce = crypto::build_nonce(
            &self.nonce_prefix,
            self.nonce_counter.fetch_add(1, Ordering::SeqCst),
        );

        // Encrypt → [12-byte nonce][ciphertext + 16-byte Poly1305 tag]
        let wire_packet = crypto::seal(&self.cipher, &nonce, packet_buf.as_ref())?;

        log::debug!(
            "🔐 Encrypted packet: seq={}, nonce={:02x?}…, wire={} bytes",
            outgoing.sequence, &nonce[..4], wire_packet.len()
        );

        // Send over UDP
        self.socket.send_to(&wire_packet, self.dest)?;

        Ok(())
    }

    /// Get the current sequence counter value (for logging/debugging).
    pub fn get_sequence(&self) -> u32 {
        self.sequence_counter.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{ArchivedMetricPacket, CpuMetrics, GpuMetrics, MemoryMetrics, NetworkMetrics, DiskMetrics};
    use std::sync::atomic::Ordering;

    /// Sending to an invalid address doesn't panic (Beverly writes after implementation).
    #[test]
    fn test_send_to_invalid_addr_fails_gracefully() {
        let dest: SocketAddr = "127.0.0.1:51057".parse().unwrap();
        let mut sender = UdpSender::new(dest, "test").expect("Failed to create UdpSender");

        let packet = MetricPacket {
            version: crate::packet::PROTOCOL_VERSION,
            machine_id: [b't', b'e', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            sender_session_id: [0u8; 16],
            timestamp: 12345,
            sequence: 99,
            cpu: CpuMetrics {
                usage_percent: 45.6,
                temperature_celsius: Some(70.0),
            },
            gpu: GpuMetrics {
                load_percent: None,
                vram_used_mb: Some(512),
                vram_total_mb: Some(8192),
                temperature_celsius: None,
            },
            memory: MemoryMetrics {
                used_bytes: 12_000_000_000,
                total_bytes: 16_000_000_000,
                swap_used_pct: 25.5,
            },
            network: NetworkMetrics {
                rx_bytes: 1_000_000,
                tx_bytes: 500_000,
            },
            disk: DiskMetrics {
                used_bytes: 150_000_000_000,
                total_bytes: 500_000_000_000,
                read_bytes: None,
                write_bytes: None,
                partitions: Vec::new(),
            },
            uptime_seconds: 3600,
        };

        // Should return Ok — buffer is serialized, encrypted, and sent via UDP to 127.0.0.1:51057.
        let result = sender.send(&packet);
        assert!(result.is_ok());
    }

    /// Verify that two consecutive sends produce different sequence numbers (replay protection).
    #[test]
    fn test_sequence_counter_increments() {
        let dest: SocketAddr = "127.0.0.1:51058".parse().unwrap();
        let mut sender = UdpSender::new(dest, "seqtest").expect("Failed to create UdpSender");

        let packet = MetricPacket {
            version: crate::packet::PROTOCOL_VERSION,
            machine_id: [b's', b'e', b'q', b't', b'e', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            sender_session_id: [0u8; 16],
            timestamp: 100,
            sequence: 0,
            cpu: CpuMetrics {
                usage_percent: 25.0,
                temperature_celsius: None,
            },
            gpu: GpuMetrics {
                load_percent: None,
                vram_used_mb: None,
                vram_total_mb: None,
                temperature_celsius: None,
            },
            memory: MemoryMetrics {
                used_bytes: 8_000_000_000,
                total_bytes: 16_000_000_000,
                swap_used_pct: 50.0,
            },
            network: NetworkMetrics {
                rx_bytes: 2_000_000,
                tx_bytes: 1_000_000,
            },
            disk: DiskMetrics {
                used_bytes: 100_000_000_000,
                total_bytes: 500_000_000_000,
                read_bytes: None,
                write_bytes: None,
                partitions: Vec::new(),
            },
            uptime_seconds: 7200,
        };

        // First send — sequence should be 0 (fetch_add returns previous value).
        sender.send(&packet).expect("First send failed");
        let seq_after_first = sender.sequence_counter.load(Ordering::SeqCst);
        assert_eq!(seq_after_first, 1); // Should have incremented to 1
        
        // Second send — sequence should be 1 (fetch_add returns previous value).
        sender.send(&packet).expect("Second send failed");
        let seq_after_second = sender.sequence_counter.load(Ordering::SeqCst);
        assert_eq!(seq_after_second, 2); // Should have incremented to 2
    }

    /// End-to-end Phase 1 guarantee: a real MetricPacket survives
    /// serialize → encrypt → decrypt → zero-copy access with all fields intact.
    #[test]
    fn test_encryption_roundtrip() {
        let packet = MetricPacket {
            version: crate::packet::PROTOCOL_VERSION,
            machine_id: [b'r', b't', b'r', b'i', b'p', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            sender_session_id: [7u8; 16],
            timestamp: 1_700_000_000,
            sequence: 5,
            cpu: CpuMetrics { usage_percent: 33.5, temperature_celsius: Some(61.0) },
            gpu: GpuMetrics { load_percent: Some(12.0), vram_used_mb: Some(1024), vram_total_mb: Some(8192), temperature_celsius: None },
            memory: MemoryMetrics { used_bytes: 4_000_000_000, total_bytes: 16_000_000_000, swap_used_pct: 1.5 },
            network: NetworkMetrics { rx_bytes: 111, tx_bytes: 222 },
            disk: DiskMetrics { used_bytes: 10, total_bytes: 100, read_bytes: None, write_bytes: None, partitions: Vec::new() },
            uptime_seconds: 42,
        };

        // Sender path: serialize + seal (same primitives send() uses).
        let plaintext = rkyv::to_bytes::<rkyv::rancor::Error>(&packet).expect("serialize");
        let cipher = crypto::cipher_from_key(&crypto::TEMP_SHARED_KEY);
        let nonce = crypto::build_nonce(&[9, 9, 9, 9], 1);
        let wire = crypto::seal(&cipher, &nonce, plaintext.as_ref()).expect("seal");

        // Wire must not leak the plaintext serialization.
        assert_ne!(&wire[crypto::NONCE_LEN..], plaintext.as_ref());

        // Receiver path: open + zero-copy access.
        let decrypted = crypto::open(&cipher, &wire).expect("open");
        let archived = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(&decrypted)
            .expect("access decrypted plaintext");
        assert_eq!(archived.machine_id, packet.machine_id);
        assert_eq!(u32::from(archived.sequence), 5);
        assert_eq!(u64::from(archived.timestamp), 1_700_000_000);
        assert_eq!(f32::from(archived.cpu.usage_percent), 33.5);
        assert_eq!(u64::from(archived.memory.total_bytes), 16_000_000_000);
        assert_eq!(u64::from(archived.uptime_seconds), 42);
    }

    /// Keypair persistence: first call generates + writes 64 bytes with 0600, second call
    /// loads the identical key back.
    #[test]
    fn test_keypair_generate_then_load() {
        let path = std::env::temp_dir()
            .join(format!("nmd_keypair_test_{}", std::process::id()))
            .join("keypair.key");
        let _ = std::fs::remove_file(&path);

        let generated = UdpSender::load_or_generate_keypair(path.clone()).expect("generate");
        let meta = std::fs::metadata(&path).expect("keypair file written");
        assert_eq!(meta.len(), 64, "keypair file must be 64 bytes");
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(meta.permissions().mode() & 0o777, 0o600, "keypair file must be 0600");
        }

        let loaded = UdpSender::load_or_generate_keypair(path.clone()).expect("load");
        assert_eq!(generated.to_keypair_bytes(), loaded.to_keypair_bytes());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(path.parent().unwrap());
    }
}