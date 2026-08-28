//! # MachineList — Shows all machines when 2+ machines are configured
//!
//! Displays a row for each machine with its full sensor panel, and a settings button at the bottom.

use crate::minimon_config::{ContentOrder, MinimonConfig};
use crate::ui::PanelWidget;
use crate::{AppMessage, remote_machine::RemoteMachine};
use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{button, column, container, divider, icon, row, scrollable, text};

/// View the machine list with all machines displayed as rows with full sensor panels
pub fn view(
    machines: &[RemoteMachine],
    content_order: &ContentOrder,
    config: &MinimonConfig,
) -> Element<'static, AppMessage> {
    let mut items: Vec<Element<'static, AppMessage>> = Vec::new();

    // Back button (left) and Settings button (right) on the same row
    let back_button = button::text("← Close").on_press(AppMessage::Back);

    let settings_button = button::custom(
        row(vec![
            icon::from_name("preferences-system-symbolic")
                .size(14)
                .into(),
            text("Settings").size(13).into(),
        ])
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(AppMessage::OpenSettings);

    let header_row = row(vec![
        container(back_button).width(Length::Fill).into(),
        container(settings_button).into(),
    ])
    .align_y(Alignment::Center);

    items.push(
        container(header_row)
            .padding([8, 16])
            .width(Length::Fill)
            .into(),
    );
    items.push(divider::horizontal::default().into());

    // Add a row for each machine
    for machine in machines {
        let machine_row = create_machine_row(machine, content_order, config);
        items.push(machine_row);
        items.push(divider::horizontal::default().into());
    }

    let content = column(items).spacing(0);

    let scrollable_content = scrollable(content).height(Length::Shrink);

    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}

/// Create a clickable row for a single machine showing its full sensor panel
fn create_machine_row(
    machine: &RemoteMachine,
    content_order: &ContentOrder,
    config: &MinimonConfig,
) -> Element<'static, AppMessage> {
    let machine_name = machine.name.clone();

    // Get the clickable sensor panel for this machine (with hover effect)
    let sensor_panel = PanelWidget::view_single_machine_clickable(
        machine,
        content_order,
        config,
        AppMessage::OpenMachineDetail(machine_name.clone()),
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
