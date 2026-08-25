//! Ring chart widget for displaying single percentage metrics.
//!
//! Adapted from minimon-applet's ring.rs, simplified for network monitor use case.

use cosmic::Renderer;
use cosmic::iced::{Point, Radians, Rectangle};
use cosmic::widget::canvas;
use cosmic::widget::canvas::{Geometry, Path, Stroke};
use crate::AppMessage;

/// Ring chart widget displaying a percentage as a circular progress indicator.
#[derive(Debug)]
pub struct RingChart {
    /// Current percentage value (0.0 - 100.0).
    pub percent: f32,
}

impl RingChart {
    /// Create a new ring chart with given value and default config.
    ///
    /// # Arguments
    /// * `percent` - The percentage to display (will be clamped to 0-100)
    pub fn new(percent: f32) -> Self {
        let percent = if percent > 100.0 { 100.0 } else if percent < 0.0 { 0.0 } else { percent };
        RingChart { percent }
    }

    /// Update the chart with a new percentage value.
    pub fn update(&mut self, value: f32) {
        let val = if value > 100.0 { 100.0 } else if value < 0.0 { 0.0 } else { value };
        self.percent = val;
    }
}

impl canvas::Program<AppMessage, cosmic::Theme> for RingChart {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &cosmic::Theme,
        bounds: Rectangle,
        _cursor: cosmic::iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // The starting point of the Ring graph (bottom/6pm)
        let start_angle = std::f32::consts::PI / 2.0;

        // Max height/width of chart/widget
        let limit = bounds.width.min(bounds.height) - 2.0;
        if limit <= 0.0 {
            return vec![frame.into_geometry()];
        }

        // Width and radius of ring based on limits (8% thickness)
        let stroke_width = 0.08 * limit;
        let radius = (limit / 2.0) - stroke_width / 2.0;
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);

        // Determine color based on percentage thresholds
        use crate::charts::theme::{MetricColor, THRESHOLD_WARN, THRESHOLD_CRIT};
        let metric_color = if self.percent < THRESHOLD_WARN {
            MetricColor::Green
        } else if self.percent < THRESHOLD_CRIT {
            MetricColor::Yellow
        } else {
            MetricColor::Red
        };
        // Convert to iced Color for stroke; background uses fixed colors (could also be themed)
        let ring_color = metric_color.as_iced_color();
        let bg_color = cosmic::iced::Color::from_rgb8(0x2e, 0x34, 0x40); // dark slate
        let inner_bg = cosmic::iced::Color::from_rgb8(0x1e, 0x1e, 0x2e); // darker

        // Draw outer background ring segment as circle (track)
        let outer_circle = Path::circle(center, radius + stroke_width / 2.0);
        frame.fill(&outer_circle, bg_color);

        // Fill inner area
        let inner_circle = Path::circle(center, radius - stroke_width / 2.0);
        frame.fill(&inner_circle, inner_bg);

        // Draw highlighted ring segment showing progress percentage
        let end_angle = start_angle + (std::f32::consts::PI * 2.0 * (self.percent / 100.0));
        let arc_path = Path::new(|p| {
            p.arc(cosmic::iced::widget::canvas::path::Arc {
                center,
                radius,
                start_angle: Radians::from(start_angle),
                end_angle: Radians::from(end_angle),
            });
        });

        frame.stroke(
            &arc_path,
            Stroke {
                style: cosmic::widget::canvas::Style::Solid(ring_color),
                width: stroke_width,
                ..Default::default()
            },
        );

        vec![frame.into_geometry()]
    }
}