//! # SettingsWindow — General configuration panel (app-wide settings)
//!
//! Provides a settings window for app-wide configuration following minimon's design:
//! - Refresh rate control (seconds)
//! - Value size control
//! - Monospace font toggle
//! - Panel spacing slider
//! - Content order reordering

use crate::minimon_config::{ContentType, MinimonConfig};
use cosmic::Element;
use cosmic::iced::Alignment;
use cosmic::widget::{button, checkbox, column, container, row, slider, text};

/// Message types for general settings window.
#[derive(Debug, Clone)]
pub enum SettingsMessage {
    /// Close the settings window and return to panel view.
    CloseWindow,
    /// Update refresh rate (in seconds as f64, will be converted to milliseconds u32)
    UpdateRefreshRate(f64),
    /// Increment refresh rate by 0.1 seconds
    IncrementRefreshRate,
    /// Decrement refresh rate by 0.1 seconds
    DecrementRefreshRate,
    /// Update value size
    UpdateValueSize(u16),
    /// Increment value size
    IncrementValueSize,
    /// Decrement value size
    DecrementValueSize,
    /// Toggle monospace font
    ToggleMonospace(bool),
    /// Update panel spacing (0=smallest, 6=largest)
    UpdatePanelSpacing(u16),
    /// Move content item up in order
    MoveContentUp(usize),
    /// Move content item down in order
    MoveContentDown(usize),
    /// No operation — used when a widget needs to return a message but no action is required.
    NoOp,
}

/// SettingsWindow renders general configuration options for the app.
///
/// Layout: refresh rate, value size, font toggle, spacing slider, content order reordering.
#[derive(Clone)]
pub struct SettingsWindow {
    /// Shared configuration manager (std::sync::Arc<RwLock>) for reading/writing machine configs.
    pub config_manager: std::sync::Arc<std::sync::RwLock<crate::config::manager::ConfigManager>>,
    /// Minimon app-wide configuration (refresh rate, value size, spacing, content order, etc.)
    pub minimon_config: MinimonConfig,
    /// Whether this window is currently visible (toggled from panel view).
    pub visible: bool,
}

impl SettingsWindow {
    /// Create a new SettingsWindow with the given shared configuration manager.
    pub fn new(
        config_manager: std::sync::Arc<std::sync::RwLock<crate::config::manager::ConfigManager>>,
    ) -> Self {
        SettingsWindow {
            config_manager,
            minimon_config: MinimonConfig::default(),
            visible: false,
        }
    }

    /// Toggle visibility of the settings window.
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    /// Update the minimon configuration
    pub fn update_config(&mut self, config: MinimonConfig) {
        self.minimon_config = config;
    }

    /// Get current config
    pub fn get_config(&self) -> &MinimonConfig {
        &self.minimon_config
    }

    /// Render the general settings view.
    pub fn view(&self) -> Element<'_, SettingsMessage> {
        view_with_config(&self.minimon_config)
    }
}

/// Standalone view function that takes owned config data.
pub fn view_with_config(minimon_config: &MinimonConfig) -> Element<'static, SettingsMessage> {
    use cosmic::widget::{divider, icon};

    // Back button
    let back_button = button::text("← Back").on_press(SettingsMessage::CloseWindow);

    // Title
    let title = text("General settings").size(24);

    // Version info
    let version_row = text("Network System Monitor for COSMIC.").size(14);

    // Refresh rate control (seconds with decimal)
    let refresh_seconds = minimon_config.refresh_rate as f64 / 1000.0;
    let refresh_row = row(vec![
        text("Refresh rate (seconds)")
            .width(cosmic::iced::Length::Fill)
            .into(),
        button::text("−")
            .on_press(SettingsMessage::DecrementRefreshRate)
            .into(),
        text(format!("{:.2}", refresh_seconds))
            .width(cosmic::iced::Length::Fixed(60.0))
            .into(),
        button::text("+")
            .on_press(SettingsMessage::IncrementRefreshRate)
            .into(),
    ])
    .spacing(8)
    .align_y(Alignment::Center);

    // Value size control
    let value_size_row = row(vec![
        text("Value size").width(cosmic::iced::Length::Fill).into(),
        button::text("−")
            .on_press(SettingsMessage::DecrementValueSize)
            .into(),
        text(format!("{}", minimon_config.value_size_default))
            .width(cosmic::iced::Length::Fixed(60.0))
            .into(),
        button::text("+")
            .on_press(SettingsMessage::IncrementValueSize)
            .into(),
    ])
    .spacing(8)
    .align_y(Alignment::Center);

    // Monospace font checkbox
    let monospace_row = row(vec![
        text("Monospace font for values")
            .width(cosmic::iced::Length::Fill)
            .into(),
        checkbox(minimon_config.monospace_values)
            .on_toggle(SettingsMessage::ToggleMonospace)
            .into(),
    ])
    .spacing(8)
    .align_y(Alignment::Center);

    // Panel spacing slider (0-6, maps to cosmic spacing units)
    let spacing_slider = slider(
        0..=6,
        minimon_config.panel_spacing,
        SettingsMessage::UpdatePanelSpacing,
    );
    let spacing_row = row(vec![
        text("Small").size(12).into(),
        spacing_slider.into(),
        text("Large").size(12).into(),
    ])
    .spacing(8)
    .align_y(Alignment::Center);

    let spacing_section = column(vec![text("Panel spacing").into(), spacing_row.into()]).spacing(4);

    // Content order reorderable list
    let content_order_label = text("Content order");

    let mut content_items: Vec<Element<'static, SettingsMessage>> = Vec::new();
    for (idx, content_type) in minimon_config.content_order.order.iter().enumerate() {
        // Skip CpuTemp - it's now combined with CPU
        if *content_type == ContentType::CpuTemp {
            continue;
        }

        let name = match content_type {
            ContentType::CpuUsage => "CPU (with Temperature)".to_string(),
            ContentType::CpuTemp => continue, // Already filtered above
            ContentType::MemoryUsage => "Memory".to_string(),
            ContentType::NetworkUsage => "Network".to_string(),
            ContentType::DiskUsage => "Disk".to_string(),
            ContentType::GpuInfo => "GPU".to_string(),
        };

        let up_button = if idx > 0 {
            button::icon(icon::from_name("go-up-symbolic"))
                .on_press(SettingsMessage::MoveContentUp(idx))
        } else {
            button::icon(icon::from_name("go-up-symbolic"))
        };

        let down_button = if idx < minimon_config.content_order.order.len() - 1 {
            button::icon(icon::from_name("go-down-symbolic"))
                .on_press(SettingsMessage::MoveContentDown(idx))
        } else {
            button::icon(icon::from_name("go-down-symbolic"))
        };

        let item_row = row(vec![
            up_button.into(),
            down_button.into(),
            text(name).width(cosmic::iced::Length::Fill).into(),
        ])
        .spacing(4)
        .align_y(Alignment::Center);

        content_items.push(item_row.into());
    }

    let content_order_list = column(content_items).spacing(4);

    // Build content column
    let content_col = column(vec![
        back_button.into(),
        title.into(),
        version_row.into(),
        divider::horizontal::default().into(),
        refresh_row.into(),
        value_size_row.into(),
        monospace_row.into(),
        spacing_section.into(),
        divider::horizontal::default().into(),
        content_order_label.into(),
        content_order_list.into(),
    ])
    .spacing(16)
    .padding([20, 16]);

    // Wrap in scrollable with max height to enable scrollbar when content is too long
    let scrollable_content =
        cosmic::widget::scrollable(content_col).height(cosmic::iced::Length::Shrink);

    container(scrollable_content)
        .width(cosmic::iced::Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}
