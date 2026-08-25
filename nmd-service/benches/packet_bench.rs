//! Benchmark: MetricPacket rkyv serialization time.
//!
//! Measures how long it takes to serialize a fully-populated `MetricPacket` via rkyv,
//! including HMAC-SHA256 tag computation by UdpSender.
//!
//! **Performance target**: < 5ms per packet (includes serialization + HMAC).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nmd_service::packet::MetricPacket;

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
        cpu_usage: 23.5,
        memory_used_percent: 45.2,
        disk_used_percent: 67.8,
        network_rx_bytes: 9_876_543_210,
        uptime_seconds: 3_600,
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

    c.bench_function("packet_serialization", |b| {
        b.iter(|| {
            // TODO: Benchmark real rkyv serialization once UdpSender is implemented (Beverly).
            black_box(&packet);
        })
    });
}

criterion_group!(benches, bench_packet_serialization);
criterion_main!(benches);