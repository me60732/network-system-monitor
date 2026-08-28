//! # Performance validation benchmarks — packet pipeline, AEAD decryption, AppState updates.
//!
//! Targets (from the performance validation plan):
//! - packet processing: < 50µs/packet (1000 packets in < 50ms)
//! - AEAD decryption:   < 20µs per packet
//! - AppState update:   < 5ms for a 10-machine update cycle
//!
//! Criterion prints measured timings to stdout; the throughput group additionally reports
//! packets/sec (elements/sec) for the 1000-packet batch.

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::{Arc, RwLock};

use cosmic_applet::config::manager::ConfigManager;
use cosmic_applet::network::test_support::{create_test_packet_full, encrypt_packet};
use cosmic_applet::network::udp_receiver::UdpReceiver;
use cosmic_applet::remote_machine::RemoteMachine;
use cosmic_applet::ui::SettingsWindow;
use cosmic_applet::{AppState, View};
use nmd_service::packet::{
    MetricPacket, ArchivedMetricPacket,
    CpuMetrics, GpuMetrics, MemoryMetrics, NetworkMetrics, DiskMetrics
};

const SECRET: &[u8; 32] = b"0123456789abcdef0123456789abcdef";

/// Fixed timestamp — benchmarks measure processing cost, not freshness policy.
const BENCH_TIMESTAMP: u64 = 1_700_000_000;

/// Convert an archived packet to the nested MetricPacket — same field mapping the
/// receiver's listen_loop performs after AEAD decryption/freshness/sequence checks pass.
fn archived_to_nested(archived: &ArchivedMetricPacket) -> MetricPacket {
    MetricPacket {
        version: archived.version.into(),
        machine_id: archived.machine_id,
        sender_session_id: archived.sender_session_id,
        timestamp: archived.timestamp.into(),
        sequence: archived.sequence.into(),
        cpu: CpuMetrics {
            usage_percent: archived.cpu.usage_percent.into(),
            temperature_celsius: archived.cpu.temperature_celsius.as_ref().map(|v| (*v).into()),
        },
        gpu: GpuMetrics {
            load_percent: archived.gpu.load_percent.as_ref().map(|v| (*v).into()),
            vram_used_mb: archived.gpu.vram_used_mb.as_ref().map(|v| (*v).into()),
            vram_total_mb: archived.gpu.vram_total_mb.as_ref().map(|v| (*v).into()),
            temperature_celsius: archived.gpu.temperature_celsius.as_ref().map(|v| (*v).into()),
        },
        memory: MemoryMetrics {
            used_bytes: archived.memory.used_bytes.into(),
            total_bytes: archived.memory.total_bytes.into(),
            swap_used_pct: archived.memory.swap_used_pct.into(),
        },
        network: NetworkMetrics {
            rx_bytes: archived.network.rx_bytes.into(),
            tx_bytes: archived.network.tx_bytes.into(),
        },
        disk: DiskMetrics {
            used_bytes: archived.disk.used_bytes.into(),
            total_bytes: archived.disk.total_bytes.into(),
            read_bytes: archived.disk.read_bytes.as_ref().map(|v| (*v).into()),
            write_bytes: archived.disk.write_bytes.as_ref().map(|v| (*v).into()),
            partitions: archived.disk.partitions.iter().map(|p| {
                nmd_service::packet::PartitionInfo {
                    mount: p.mount.to_string(),
                    total: p.total.into(),
                    used: p.used.into(),
                }
            }).collect(),
        },
        uptime_seconds: archived.uptime_seconds.into(),
    }
}

/// Extract the machine name from a nested packet's null-padded machine_id.
fn machine_name(packet: &MetricPacket) -> String {
    let len = packet.machine_id.iter().position(|&b| b == 0).unwrap_or(20);
    std::str::from_utf8(&packet.machine_id[..len]).unwrap_or("unknown").to_string()
}

/// bench_packet_processing_throughput: full receive pipeline (rkyv access → AEAD decrypt →
/// nested packet conversion → machine state update) over 1000 pre-serialized valid packets.
/// Target: < 50ms total (< 50µs/packet). Criterion reports elements/sec = packets/sec.
fn bench_packet_processing_throughput(c: &mut Criterion) {
    let receiver = UdpReceiver::new(0, None).expect("bind receiver");
    let buffers: Vec<Vec<u8>> = (1..=1000u32)
        .map(|seq| {
            let pkt = create_test_packet_full(
                "bench-machine",
                50.0,
                4_000_000_000,
                8_000_000_000,
                seq,
                BENCH_TIMESTAMP,
            );
            encrypt_packet(pkt, SECRET)
        })
        .collect();

    let mut group = c.benchmark_group("packet_processing");
    group.throughput(Throughput::Elements(buffers.len() as u64));
    group.bench_function("process_1000_valid_packets", |b| {
        b.iter(|| {
            let mut machines: HashMap<String, RemoteMachine> = HashMap::new();
            for buf in &buffers {
                // Decrypt + verify AEAD tag
                let plaintext = receiver.decrypt_packet(buf.as_slice())
                    .expect("AEAD decryption should succeed");
                
                let archived = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(
                    black_box(plaintext.as_slice()),
                )
                .expect("access");
                let packet = archived_to_nested(archived);
                let name = machine_name(&packet);
                machines
                    .entry(name.clone())
                    .or_insert_with(|| RemoteMachine::new(name))
                    .update_from_packet(&packet);
            }
            black_box(machines)
        })
    });
    group.finish();
}

/// bench_aead_decryption_time: ChaCha20-Poly1305 decrypt + verify tag on a typical packet
/// (target < 20µs), plus the full deserialization path for comparison.
fn bench_aead_decryption_time(c: &mut Criterion) {
    let receiver = UdpReceiver::new(0, None).expect("bind receiver");
    let packet = create_test_packet_full(
        "bench-machine",
        42.0,
        4_000_000_000,
        8_000_000_000,
        1,
        BENCH_TIMESTAMP,
    );
    let buffer = encrypt_packet(packet, SECRET);
    let archived = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(&buffer)
        .expect("access");

    c.bench_function("aead_decryption_verify_tag", |b| {
        b.iter(|| {
            let plaintext = receiver.decrypt_packet(black_box(&buffer))
                .expect("AEAD decryption should succeed");
            black_box(plaintext);
        })
    });

    // Comparison: full deserialization path (decrypt + zero-copy access + owned nested conversion).
    c.bench_function("full_deserialization_path", |b| {
        b.iter(|| {
            let plaintext = receiver.decrypt_packet(black_box(&buffer))
                .expect("AEAD decryption should succeed");
            let a = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(
                black_box(plaintext.as_slice()),
            )
            .expect("access");
            black_box(archived_to_nested(a))
        })
    });
}

/// bench_appstate_update_time: 10 machines updated in one cycle through the shared
/// Arc<RwLock<AppState>> write path the UDP receiver uses. Target: < 5ms per full cycle.
fn bench_appstate_update_time(c: &mut Criterion) {
    let config_manager = Arc::new(RwLock::new(ConfigManager::default()));
    let settings_window = SettingsWindow::new(config_manager.clone());

    let names: Vec<String> = (0..10).map(|i| format!("machine-{i:02}")).collect();
    let mut machines = HashMap::new();
    for name in &names {
        machines.insert(name.clone(), RemoteMachine::new(name.clone()));
    }
    let state = Arc::new(RwLock::new(AppState {
        config_manager,
        current_view: View::Panel,
        settings_window,
        machines,
    }));

    // One pre-built packet per machine, simulating 10 simultaneous arrivals.
    let packets: Vec<(String, MetricPacket)> = names
        .iter()
        .map(|name| {
            (
                name.clone(),
                create_test_packet_full(name, 33.0, 4_000_000_000, 8_000_000_000, 1, BENCH_TIMESTAMP),
            )
        })
        .collect();

    c.bench_function("appstate_update_10_machines", |b| {
        b.iter(|| {
            let mut st = state.write().unwrap();
            for (name, packet) in &packets {
                st.machines
                    .get_mut(name)
                    .expect("machine exists")
                    .update_from_packet(black_box(packet));
            }
        })
    });
}

criterion_group!(
    benches,
    bench_packet_processing_throughput,
    bench_aead_decryption_time,
    bench_appstate_update_time
);
criterion_main!(benches);
