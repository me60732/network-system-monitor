//! Benchmark: CPU collection performance.
//!
//! Measures the time to collect full CPU statistics (aggregate + per-core breakdown) via sysinfo.
//! Target: < 50ms for the entire metrics suite including this call.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use metrics_core::cpu;

fn bench_cpu_collect(c: &mut Criterion) {
    c.bench_function("bench_cpu_collect", |b| {
        b.iter(|| black_box(cpu::collect()));
    });
}

criterion_group!(benches, bench_cpu_collect);
criterion_main!(benches);
