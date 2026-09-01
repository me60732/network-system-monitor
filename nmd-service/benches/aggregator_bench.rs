//! Benchmark: MetricsAggregator end-to-end aggregation overhead.
//!
//! Measures the time to call `metrics_core::collect_all()` and pack results into a `MetricPacket`.
//!
//! **Performance target**: < 5ms per aggregate cycle (includes all metric collection).

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use nmd_service::MetricsAggregator;

fn bench_aggregate_overhead(c: &mut Criterion) {
    let machine_id = "test-host";
    let mut aggregator = MetricsAggregator::new(machine_id);

    c.bench_function("aggregate_overhead", |b| {
        b.iter(|| {
            // TODO: Benchmark real aggregation once metrics-core is implemented (Beverly).
            black_box(aggregator.aggregate());
        })
    });
}

criterion_group!(benches, bench_aggregate_overhead);
criterion_main!(benches);
