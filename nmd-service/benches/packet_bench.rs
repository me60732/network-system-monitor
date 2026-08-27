//! Benchmark: MetricPacket rkyv serialization time.
//!
//! Measures how long it takes to serialize a fully-populated `MetricPacket` via rkyv,
//! including HMAC-SHA256 tag computation by UdpSender.
//!
//! **Performance target**: < 5ms per packet (includes serialization + HMAC).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nmd_service::packet::{MetricPacket, CpuMetrics, GpuMetrics, MemoryMetrics, NetworkMetrics, DiskMetrics};

fn bench_packet_serialization(c: &mut Criterion) {
    let mut machine_id_bytes = [0u8; 20];
    let src = "pluto".as_bytes();
    let len = src.len().min(20);
    machine_id_bytes[..len].copy_from_slice(&src[..len]);
    
    let packet = MetricPacket {
        version: nmd_service::PROTOCOL_VERSION,
        machine_id: machine_id_bytes,
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
        hmac_tag: [0u8; 32],
    };

    c.bench_function("packet_serialization", |b| {
        b.iter(|| {
            // TODO: Benchmark real rkyv serialization once UdpSender is implemented (Beverly).
            black_box(&packet);
        })
    });
}

criterion_group!(benches, bench_packet_serialization);
criterion_main!(benches);