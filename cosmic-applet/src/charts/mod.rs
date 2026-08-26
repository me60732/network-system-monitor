//! Chart rendering module for metric visualization.
//!
//! Provides ring charts and themed color palettes.

pub mod ring;
pub mod theme;

// Import minimon's chart colors from config module
pub use crate::minimon_config::{ChartColors, ChartKind, DeviceKind, ColorVariant};

/// Minimon chart color system converted to Iced Colors for rendering.
#[derive(Debug, Clone, Copy)]
pub struct ChartColorsIced {
    pub color1: cosmic::iced::Color,
    pub color2: cosmic::iced::Color,
    pub color3: cosmic::iced::Color,
    pub color4: cosmic::iced::Color,
}

impl From<ChartColors> for ChartColorsIced {
    fn from(colors: ChartColors) -> Self {
        fn to_iced_color(srgba: cosmic::cosmic_theme::palette::Srgba<u8>) -> cosmic::iced::Color {
            cosmic::iced::Color {
                r: srgba.color.red as f32 / 255.0,
                g: srgba.color.green as f32 / 255.0,
                b: srgba.color.blue as f32 / 255.0,
                a: srgba.alpha as f32 / 255.0,
            }
        }

        ChartColorsIced {
            color1: to_iced_color(colors.background),
            color2: to_iced_color(colors.frame),
            color3: to_iced_color(colors.text),
            color4: to_iced_color(colors.graph1),
        }
    }
}

// Export minimon's chart widgets
pub use ring::RingChart;

/// Trait for chart widgets that can render metrics.
pub trait Chart {
    /// Render the chart with given dimensions.
    fn view<'a, Message>(&self, width: u32, height: u32) -> cosmic::Element<'a, Message>
    where
        Message: 'a;

    /// Update the chart with a new value.
    fn update(&mut self, value: f32);
}