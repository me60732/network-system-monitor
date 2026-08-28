//! # Test & benchmark support — shared packet construction and encryption helpers.
//!
//! Single source of truth for building valid `MetricPacket`s and rkyv-serializing + encrypting
//! them into the ChaCha20-Poly1305 wire format. Used by the unit tests in `udp_receiver`, the
//! integration tests in `tests/integration_tests.rs`, and the criterion benchmarks in
//! `benches/performance_bench.rs`.
//! Not part of the public applet API (`#[doc(hidden)]` at the module declaration).

use nmd_service::crypto;
use nmd_service::packet::{
    CpuMetrics, DiskMetrics, GpuMetrics, MemoryMetrics, MetricPacket, NetworkMetrics,
    PartitionInfo,
};
use std::sync::atomic::{AtomicU64, Ordering};

/// Process-wide nonce counter — guarantees every test/bench wire packet gets a unique nonce.
static TEST_NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Current Unix time in seconds — convenience for freshness-sensitive test packets.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Create a valid MetricPacket with nested struct groups (sequence 42, current timestamp).
pub fn create_test_packet(
    machine_name: &str,
    cpu_usage: f32,
    memory_used: u64,
    memory_total: u64,
) -> MetricPacket {
    create_test_packet_full(machine_name, cpu_usage, memory_used, memory_total, 42, unix_now())
}

/// Full-control variant: caller supplies the sequence number and timestamp.
/// Needed by replay-protection and clock-skew tests where those fields drive acceptance.
pub fn create_test_packet_full(
    machine_name: &str,
    cpu_usage: f32,
    memory_used: u64,
    memory_total: u64,
    sequence: u32,
    timestamp: u64,
) -> MetricPacket {
    let mut machine_id_bytes = [0u8; 20];
    let src = machine_name.as_bytes();
    let len = src.len().min(20);
    machine_id_bytes[..len].copy_from_slice(&src[..len]);

    // SEC-03: Generate fixed test session ID (reproducible for tests)
    let sender_session_id: [u8; 16] = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
        0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10,
    ];

    MetricPacket {
        version: nmd_service::packet::PROTOCOL_VERSION,
        machine_id: machine_id_bytes,
        sender_session_id,
        timestamp,
        sequence,

        cpu: CpuMetrics {
            usage_percent: cpu_usage,
            temperature_celsius: Some(65.0),
        },

        gpu: GpuMetrics {
            load_percent: Some(50.0),
            vram_used_mb: Some(2048),  // 2GB
            vram_total_mb: Some(8192), // 8GB
            temperature_celsius: Some(75.0),
        },

        memory: MemoryMetrics {
            used_bytes: memory_used,
            total_bytes: memory_total,
            swap_used_pct: 10.5,
        },

        network: NetworkMetrics {
            rx_bytes: 1_000_000,
            tx_bytes: 500_000,
        },

        disk: DiskMetrics {
            used_bytes: 320_000_000_000,
            total_bytes: 500_000_000_000,
            read_bytes: Some(50_000_000),
            write_bytes: Some(30_000_000),
            partitions: vec![PartitionInfo {
                mount: "/".to_string(),
                total: 500_000_000_000,
                used: 320_000_000_000,
            }],
        },

        uptime_seconds: 86400, // 1 day
    }
}

/// Serialize a packet via rkyv and encrypt it into the wire format
/// `[12-byte nonce][ciphertext + 16-byte Poly1305 tag]` under the given 32-byte key.
/// Mirrors nmd-service's sender path; each call uses a fresh counter-derived nonce.
pub fn encrypt_packet(packet: MetricPacket, key: &[u8; 32]) -> Vec<u8> {
    let plaintext = rkyv::to_bytes::<rkyv::rancor::Error>(&packet)
        .expect("Serialization should succeed");

    let cipher = crypto::cipher_from_key(key);
    let nonce = crypto::build_nonce(
        b"TEST", // fixed prefix — distinct from real senders' random prefixes
        TEST_NONCE_COUNTER.fetch_add(1, Ordering::SeqCst),
    );
    crypto::seal(&cipher, &nonce, plaintext.as_ref()).expect("Encryption should succeed")
}

/// Convenience: encrypt under the Phase 1 temporary shared key the receiver uses.
pub fn encrypt_packet_default(packet: MetricPacket) -> Vec<u8> {
    encrypt_packet(packet, &crypto::TEMP_SHARED_KEY)
}
