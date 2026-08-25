//! Benchmark: PanelWidget render time from metrics data.
//!
//! Measures how long it takes to render the single-line Cosmic panel widget from a full set of
//! metric values (CPU, memory, disk, network, uptime, GPU VRAM, temperature).
//!
//! **Performance target**: < 10ms per render cycle.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use cosmic_applet::panel_widget::{PanelWidget, PanelWidget as _};
use cosmic_applet::config_manager::ConfigManager;

fn bench_panel_render(c: &mut Criterion) {
    let config = ConfigManager::default();
    let widget = PanelWidget::new(config);

    c.bench_function("panel_render", |b| {
        b.iter(|| {
            // TODO: Benchmark real panel rendering once UI is implemented (Beverly).
            black_box(&widget);
        })
    });
}

criterion_group!(benches, bench_panel_render);
criterion_main!(benches);