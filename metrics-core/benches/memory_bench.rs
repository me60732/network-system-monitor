//! Benchmark: Memory collection performance.
//!
//! Measures the time to collect memory and swap statistics via sysinfo (reads /proc/meminfo).
//! Target: < 50ms for the entire metrics suite including this call.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use metrics_core::memory;

fn bench_memory_collect(c: &mut Criterion) {
    c.bench_function("bench_memory_collect", |b| {
        b.iter(|| black_box(memory::collect()));
    });
}

criterion_group!(benches, bench_memory_collect);
criterion_main!(benches);
