//! Simple progress bar widget for grid machine cards.

use cosmic::iced::{Length, Alignment};
use cosmic::iced::widget::{container, text};
use cosmic::Element;
use crate::charts::Chart;

/// Horizontal progress bar showing percentage completion.
pub struct ProgressBar {
    /// Current percentage (0.0 - 100.0).
    value: f32,
}

impl ProgressBar {
    /// Create a new progress bar with given value.
    pub fn new(value: f32) -> Self {
        let value = value.clamp(0.0, 100.0);
        ProgressBar { value }
    }

    /// Update the progress bar with a new value.
    pub fn update(&mut self, value: f32) {
        self.value = value.clamp(0.0, 100.0);
    }
}

impl Chart for ProgressBar {
    /// Render the progress bar as a cosmic Element.
    ///
    /// For now, we just show the percentage as text. This can be improved later to show an actual bar.
    fn view<'a, Message>(&self, width: u32, height: u32) -> Element<'a, Message>
    where
        Message: 'a,
    {
        use cosmic::widget::text;
        let text_str = format!("{:.0}%", self.value);
        container(text(text_str))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
    }

    fn update(&mut self, value: f32) {
        self.value = value.clamp(0.0, 100.0);
    }
}