use cosmic::Renderer;
use cosmic::iced::Point;
use cosmic::iced::Radians;
use cosmic::iced::Rectangle;
use cosmic::iced::mouse::Cursor;
use cosmic::theme;
use cosmic::widget::canvas;
use cosmic::widget::canvas::Geometry;

use cosmic::widget::canvas::Path;
use cosmic::widget::canvas::Text;
use cosmic::widget::canvas::path::Arc;

use std::f32::consts::PI;

use crate::AppMessage as Message;
use crate::minimon_config::ChartColors;

use super::ChartColorsIced;

#[derive(Debug)]
pub struct RingChart {
    // How much if the ring is filled. 0..100
    pub percent: f32,

    //Text to display inside, if any
    pub text: String,
    pub colors: ChartColorsIced,
}

impl RingChart {
    /// Create a ring chart with auto-formatted percentage text
    /// Values >= 10 show 1 decimal place, values < 10 show 2 decimal places
    pub fn new(percent: f32, colors: &ChartColors) -> Self {
        let clamped_percent = if percent <= 100.0 { percent } else { 100.0 };
        let text = Self::format_value(clamped_percent);
        RingChart {
            percent: clamped_percent,
            text,
            colors: (*colors).into(),
        }
    }

    /// Create a ring chart with custom text
    pub fn new_with_text(percent: f32, text: &str, colors: &ChartColors) -> Self {
        RingChart {
            percent: if percent <= 100.0 { percent } else { 100.0 },
            text: text.to_string(),
            colors: (*colors).into(),
        }
    }

    /// Format a numeric value for chart display, keeping it compact
    /// - Value = 100: show "100" (no decimal)
    /// - Values >= 10: 1 decimal place (e.g., "45.3")
    /// - Values < 10: 2 decimal places (e.g., "9.99")
    fn format_value(value: f32) -> String {
        if (value - 100.0).abs() < 0.01 {
            "100".to_string()
        } else if value >= 10.0 {
            format!("{:.1}", value)
        } else {
            format!("{:.2}", value)
        }
    }
}

impl canvas::Program<Message, theme::Theme> for RingChart {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &theme::Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // The starting poing of the Ring graph, bottom/6pm
        let starting_point = PI / 2.0;

        // Max height/width of chart/widget. Side length in a square
        let limit = bounds.width.min(bounds.height) - 2.0;

        // Width and radius of ring - stroke width 0.06
        let stroke_width = 0.06 * limit;
        let radius = (limit / 2.0) - stroke_width / 2.0;
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);

        // Pick background ring colour based on light/dark theme
        let grey = if theme.theme_type.is_dark() {
            cosmic::iced::Color::from_rgb(0.25, 0.25, 0.25)
        } else {
            cosmic::iced::Color::from_rgb(0.72, 0.72, 0.72)
        };

        // Draw outer background ring segment as circle
        let outer_circle = Path::circle(center, radius + (stroke_width / 2.0));
        frame.fill(&outer_circle, grey);

        // Fill background color inside ring with theme awareness
        let inner_circle = Path::circle(center, radius - stroke_width / 2.0);
        let inner_bg = if theme.theme_type.is_dark() {
            self.colors.color1 // keep existing dark background
        } else {
            // In light theme, use a near-white inner fill
            cosmic::iced::Color::from_rgb(0.95, 0.95, 0.95)
        };
        frame.fill(&inner_circle, inner_bg);

        // Define threshold colors for graduated ring display
        let green = cosmic::iced::Color::from_rgb(0.0, 0.8, 0.2);
        let orange = cosmic::iced::Color::from_rgb(1.0, 0.6, 0.0);
        let red = cosmic::iced::Color::from_rgb(1.0, 0.2, 0.2);

        // Draw ring with graduated colors based on thresholds (green 0-60%, orange 60-80%, red 80-100%)
        // Draw each threshold segment separately with its own color

        // Green segment: 0% to min(60%, current%)
        if self.percent > 0.0 {
            let green_end = self.percent.min(60.0);
            let green_ring = Path::new(|p| {
                p.arc(Arc {
                    center,
                    radius,
                    start_angle: Radians::from(starting_point),
                    end_angle: Radians::from(starting_point + (PI * 2.0 * (green_end / 100.0))),
                });
            });
            frame.stroke(
                &green_ring,
                canvas::Stroke {
                    style: canvas::Style::Solid(green),
                    width: stroke_width,
                    ..Default::default()
                },
            );
        }

        // Orange segment: 60% to min(80%, current%)
        if self.percent > 60.0 {
            let orange_start = 60.0;
            let orange_end = self.percent.min(80.0);
            let orange_ring = Path::new(|p| {
                p.arc(Arc {
                    center,
                    radius,
                    start_angle: Radians::from(
                        starting_point + (PI * 2.0 * (orange_start / 100.0)),
                    ),
                    end_angle: Radians::from(starting_point + (PI * 2.0 * (orange_end / 100.0))),
                });
            });
            frame.stroke(
                &orange_ring,
                canvas::Stroke {
                    style: canvas::Style::Solid(orange),
                    width: stroke_width,
                    ..Default::default()
                },
            );
        }

        // Red segment: 80% to current%
        if self.percent > 80.0 {
            let red_start = 80.0;
            let red_ring = Path::new(|p| {
                p.arc(Arc {
                    center,
                    radius,
                    start_angle: Radians::from(starting_point + (PI * 2.0 * (red_start / 100.0))),
                    end_angle: Radians::from(starting_point + (PI * 2.0 * (self.percent / 100.0))),
                });
            });
            frame.stroke(
                &red_ring,
                canvas::Stroke {
                    style: canvas::Style::Solid(red),
                    width: stroke_width,
                    ..Default::default()
                },
            );
        }

        // Create centered text object with smaller size (0.82 instead of 0.93 to compensate for larger canvas)
        let text = Text {
            content: self.text.clone(),
            position: center,
            color: if theme.theme_type.is_dark() {
                self.colors.color2
            } else {
                cosmic::iced::Color::from_rgb(0.1, 0.1, 0.1)
            },
            size: cosmic::iced::Pixels(radius * 0.82),
            align_x: cosmic::iced::alignment::Horizontal::Center.into(),
            align_y: cosmic::iced::alignment::Vertical::Center.into(),
            ..Default::default()
        };

        frame.fill_text(text);

        vec![frame.into_geometry()]
    }
}
