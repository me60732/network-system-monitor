//! Benchmark: MetricPacket rkyv serialization + ChaCha20-Poly1305 encryption time.
//!
//! Measures the sender-side hot path: rkyv serialization of a fully-populated `MetricPacket`,
//! AEAD encryption into the wire format, and decryption for comparison.
//!
//! **Performance target**: < 5ms per packet (serialization + encryption combined).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nmd_service::crypto;
use nmd_service::packet::{MetricPacket, CpuMetrics, GpuMetrics, MemoryMetrics, NetworkMetrics, DiskMetrics};

fn bench_packet_pipeline(c: &mut Criterion) {
    let mut machine_id_bytes = [0u8; 20];
    let src = "pluto".as_bytes();
    let len = src.len().min(20);
    machine_id_bytes[..len].copy_from_slice(&src[..len]);
    
    let packet = MetricPacket {
        version: nmd_service::PROTOCOL_VERSION,
        machine_id: machine_id_bytes,
        sender_session_id: [0xAB; 16],
        timestamp: 1234567890,
        sequence: 42,
        
        cpu: CpuMetrics {
            usage_percent: 23.5,
            temperature_celsius: Some(65.0),
        },
        
        gpu: GpuMetrics {
            load_percent: None,
            vram_used_mb: Some(512),
            vram_total_mb: None,
            temperature_celsius: None,
        },
        
        memory: MemoryMetrics {
            used_bytes: 8_000_000_000,  // 8 GB used
            total_bytes: 16_000_000_000, // 16 GB total
            swap_used_pct: 0.0,
        },
        
        network: NetworkMetrics {
            rx_bytes: 9_876_543_210,
            tx_bytes: 5_432_109_876,
        },
        
        disk: DiskMetrics {
            used_bytes: 300_000_000_000,   // 300 GB used
            total_bytes: 500_000_000_000,  // 500 GB total
            read_bytes: None,              // Phase 2: IO stats (sysinfo doesn't expose these)
            write_bytes: None,             // Phase 2: IO stats (sysinfo doesn't expose these)
            partitions: Vec::new(),
        },
        
        uptime_seconds: 3_600,
    };

    let cipher = crypto::cipher_from_key(&crypto::TEMP_SHARED_KEY);
    let nonce = crypto::build_nonce(&[1, 2, 3, 4], 7);

    // Serialization alone — baseline for the rkyv cost.
    c.bench_function("packet_serialization", |b| {
        b.iter(|| {
            black_box(rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&packet)).expect("serialize"))
        })
    });

    // Encryption alone on a pre-serialized buffer — isolates the AEAD cost.
    let plaintext = rkyv::to_bytes::<rkyv::rancor::Error>(&packet).expect("serialize");
    c.bench_function("packet_encryption", |b| {
        b.iter(|| {
            black_box(crypto::seal(&cipher, &nonce, black_box(plaintext.as_ref())).expect("seal"))
        })
    });

    // Combined serialize + encrypt — the per-packet work UdpSender::send performs.
    c.bench_function("packet_serialize_encrypt", |b| {
        b.iter(|| {
            let buf = rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&packet)).expect("serialize");
            black_box(crypto::seal(&cipher, &nonce, buf.as_ref()).expect("seal"))
        })
    });

    // Decryption — receiver-side AEAD open on the same wire packet.
    let wire = crypto::seal(&cipher, &nonce, plaintext.as_ref()).expect("seal");
    c.bench_function("packet_decryption", |b| {
        b.iter(|| black_box(crypto::open(&cipher, black_box(&wire)).expect("open")))
    });
}

criterion_group!(benches, bench_packet_pipeline);
criterion_main!(benches);