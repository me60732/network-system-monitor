//! # MachineList — Shows all machines when 2+ machines are configured
//!
//! Displays a row for each machine with its full sensor panel, and a settings button at the bottom.

use crate::minimon_config::{ContentOrder, MachineSensorConfig, MinimonConfig};
use crate::ui::PanelWidget;
use crate::{AppMessage, remote_machine::RemoteMachine};
use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{button, column, container, divider, icon, row, scrollable, text};

/// View the machine list with all machines displayed as rows with full sensor panels
pub fn view(
    machines: &[RemoteMachine],
    content_order: &ContentOrder,
    local_machine_name: &str,
    local_sensor_config: &MachineSensorConfig,
    config_manager: &crate::config::manager::ConfigManager,
    minimon_config: &MinimonConfig,
) -> Element<'static, AppMessage> {
    let mut items: Vec<Element<'static, AppMessage>> = Vec::new();

    // Back button (left) and Settings button (right) on the same row
    let system_monitor_button = button::custom(
        row(vec![
            text("COSMIC System Monitor").size(14).into(),
            icon::from_name("send-to-symbolic").size(14).into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .on_press(AppMessage::LaunchSystemMonitor)
    .width(Length::Fill);

    let settings_button_only = button::custom(
        row(vec![
            icon::from_name("preferences-system-symbolic")
                .size(14)
                .into(),
            text("General Settings").size(13).into(),
        ])
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(AppMessage::OpenGeneralSettings);

    let header_row = container(
        row(vec![
            system_monitor_button.into(),
            settings_button_only.into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([8, 16])
    .width(Length::Fill);

    items.push(header_row.into());
    items.push(divider::horizontal::default().into());

    // Add a row for each machine
    for machine in machines {
        let sensor_config = if machine.name == local_machine_name {
            local_sensor_config.clone()
        } else {
            config_manager
                .machines
                .iter()
                .find(|m| m.name == machine.name)
                .map(|m| m.sensor_config.clone())
                .unwrap_or_default()
        };
        let display = crate::ui::panel_widget::GlobalDisplayConfig::from_minimon(minimon_config);
        let machine_row = create_machine_row(machine, content_order, &sensor_config, &display);
        items.push(machine_row);
        items.push(divider::horizontal::default().into());
    }

    let content = column(items).spacing(0);

    let scrollable_content = scrollable(content).height(Length::Shrink);

    container(scrollable_content)
        .width(Length::Shrink)
        .max_height(600.0)
        .into()
}

/// Create a clickable row for a single machine showing its full sensor panel
fn create_machine_row(
    machine: &RemoteMachine,
    content_order: &ContentOrder,
    sensor_config: &MachineSensorConfig,
    display: &crate::ui::panel_widget::GlobalDisplayConfig,
) -> Element<'static, AppMessage> {
    let machine_name = machine.name.clone();

    // Get the clickable sensor panel for this machine (with hover effect)
    let sensor_panel = PanelWidget::view_single_machine_clickable(
        machine,
        content_order,
        sensor_config,
        AppMessage::OpenMachineDetail(machine_name.clone()),
        display,
    );

    // Build the row with machine name above the sensor panel
    column(vec![
        container(text(machine_name).size(14))
            .padding([4, 12, 0, 12])
            .into(),
        container(sensor_panel)
            .padding([4, 0])
            .center_x(Length::Fill)
            .into(),
    ])
    .spacing(4)
    .width(Length::Fill)
    .into()
}
