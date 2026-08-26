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
    /// - Values >= 10: 1 decimal place (e.g., "45.3")
    /// - Values < 10: 2 decimal places (e.g., "9.99")
    fn format_value(value: f32) -> String {
        if value >= 10.0 {
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
        _theme: &theme::Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // The starting poing of the Ring graph, bottom/6pm
        let starting_point = PI / 2.0;

        // Max height/width of chart/widget. Side length in a square
        let limit = bounds.width.min(bounds.height)-2.0;

        // Width and radius of ring - stroke width 0.06
        let stroke_width = 0.06 * limit;
        let radius = (limit / 2.0) - stroke_width / 2.0;
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);

        // Use dark grey for rings
        let grey = cosmic::iced::Color::from_rgb(0.25, 0.25, 0.25);

        // Draw outer background ring segment as circle
        let outer_circle = Path::circle(center, radius+(stroke_width / 2.0));
        frame.fill(&outer_circle, grey);

        // Fill background color inside ring
        let inner_circle = Path::circle(center, radius - stroke_width / 2.0);
        frame.fill(&inner_circle, self.colors.color1);

        // Draw highlighted ring segment showing status/percentage
        let ring = Path::new(|p| {
            p.arc(Arc {
                center,
                radius,
                start_angle: Radians::from(starting_point),
                end_angle: Radians::from(starting_point + (PI * 2.0 * (self.percent / 100.0))),
            });
        });
        
        frame.stroke(
            &ring,
            canvas::Stroke {
                style: canvas::Style::Solid(grey),
                width: stroke_width,
                ..Default::default()
            },
        );

        // Create centered text object with smaller size (0.82 instead of 0.93 to compensate for larger canvas)
        let text = Text {
            content: self.text.clone(),
            position: center,
            color: self.colors.color2,
            size: cosmic::iced::Pixels(radius * 0.82),
            align_x: cosmic::iced::alignment::Horizontal::Center.into(),
            align_y: cosmic::iced::alignment::Vertical::Center.into(),
            ..Default::default()
        };

        frame.fill_text(text);

        vec![frame.into_geometry()]
    }
}
