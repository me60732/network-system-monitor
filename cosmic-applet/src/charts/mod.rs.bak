//! Chart rendering module for metric visualization.
//!
//! Provides ring charts, progress bars, and themed color palettes.

pub mod ring;
pub mod progress_bar;
pub mod theme;

pub use ring::RingChart;
pub use progress_bar::ProgressBar;
pub use theme::{MetricColor, THRESHOLD_WARN, THRESHOLD_CRIT, StatusIndicator, format_network_rate, format_uptime};

/// Trait for chart widgets that can render metrics.
pub trait Chart {
    /// Render the chart with given dimensions.
    fn view<'a, Message>(&self, width: u32, height: u32) -> cosmic::Element<'a, Message>
    where
        Message: 'a;

    /// Update the chart with a new value.
    fn update(&mut self, value: f32);
}