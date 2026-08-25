//! Benchmark: GridWindow update time with N machine rows.
//!
//! Measures the time to populate and render a grid window containing all configured remote machines,
//! including progress bars, status indicators, and color-coded thresholds at 60%/80%.
//!
//! **Performance target**: < 50ms for full grid update with complete remote machine list.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use cosmic_applet::grid_window::GridWindow;

fn bench_grid_update(c: &mut Criterion) {
    let mut grid = GridWindow::new();
    // Simulate a realistic number of machines (e.g., 10 remote hosts).
    let names: Vec<String> = (0..10)
        .map(|i| format!("machine-{}", i))
        .collect();
    grid.populate_from_config(&names);

    c.bench_function("grid_update", |b| {
        b.iter(|| {
            // TODO: Benchmark real grid rendering + update once UI is implemented (Beverly).
            black_box(&grid);
        })
    });
}

criterion_group!(benches, bench_grid_update);
criterion_main!(benches);