//! Benchmark: Full metrics collection suite.
//!
//! Measures end-to-end time to collect ALL metric types (CPU, memory, disk, network, uptime, GPU, temperature).
//! This is the primary performance gate for real-time panel updates.
//! Target: < 50ms total on typical Linux hardware.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use metrics_core::collect_all;

fn bench_full_suite(c: &mut Criterion) {
    c.bench_function("bench_full_metrics_collection", |b| {
        b.iter(|| black_box(collect_all()));
    });
}

criterion_group!(benches, bench_full_suite);
criterion_main!(benches);
