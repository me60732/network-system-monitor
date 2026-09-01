//! # MachineDetailView — Per-machine metric detail view
//!
//! Shows compact panel at top with visible sensors, then details below for everything else:
//! - Back button (< Back) top-left
//! - Machine name as title
//! - Compact sensor panel (shows what's currently visible with charts)
//! - Detail sections for enabled sensors that don't have charts visible

use crate::AppMessage;
use crate::charts::ring::RingChart;
use crate::minimon_config::{ChartColors, ChartKind, DeviceKind};
use crate::ui::panel_widget::PanelWidget;
use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{button, canvas, column, container, divider, icon, row, scrollable, text};

/// View the machine detail view for a specific machine with live data
pub fn view(
    machine_config: &crate::config::manager::MachineConfig,
    remote_machine: &crate::remote_machine::RemoteMachine,
    minimon_config: &crate::minimon_config::MinimonConfig, // global settings only
    is_local: bool,                                        // if true, hide the Remove button
) -> Element<'static, AppMessage> {
    // Clone all data upfront to make Element 'static
    let machine_name_for_title = machine_config.name.clone();
    let machine_name_for_remove = machine_config.name.clone();

    // Get per-machine sensor config from machine_config
    let sensor_config = &machine_config.sensor_config;

    // Check what's visible in the panel (has charts) using per-machine sensor_config
    let cpu_chart_visible = sensor_config.cpu.chart_visible();
    let cpu_temp_chart_visible = sensor_config.cputemp.chart_visible();
    let memory_chart_visible = sensor_config.memory.chart_visible();
    let network_chart_visible = sensor_config.network1.chart_visible();
    let _disk_chart_visible = false; // Disk doesn't have panel chart yet

    // Fix GPU visibility: use three independent booleans instead of one combined check
    let gpu_load_chart_visible = sensor_config.gpu.usage.chart_visible();
    let gpu_vram_chart_visible = sensor_config.gpu.vram.chart_visible();
    let gpu_temp_chart_visible = sensor_config.gpu.temp.chart_visible();

    // Clone sensor data for rendering details
    let cpu_percent = remote_machine.sensors.cpu.usage_percent;
    let memory_percent = remote_machine.sensors.memory.usage_percent();
    let memory_gb = remote_machine.sensors.memory.used_bytes as f64 / 1_073_741_824.0;
    let memory_swap_pct = remote_machine.sensors.memory.swap_used_pct;
    let network_rx_kbps = remote_machine.sensors.network.rx_bytes_per_sec as f64 / 1024.0;
    let network_tx_kbps = remote_machine.sensors.network.tx_bytes_per_sec as f64 / 1024.0;
    let disk_write_kbps = remote_machine.sensors.disk.write_bytes_per_sec as f64 / 1024.0;
    let disk_read_kbps = remote_machine.sensors.disk.read_bytes_per_sec as f64 / 1024.0;
    let disk_partitions = remote_machine.sensors.disk.partitions.clone();
    let gpu_percent = remote_machine.sensors.gpu.usage_percent();
    let gpu_gb = remote_machine.sensors.gpu.vram_used_bytes as f64 / 1_073_741_824.0;
    let temp_celsius = remote_machine.sensors.temperature.celsius;
    let uptime_seconds = remote_machine.sensors.uptime_seconds;

    let content_order = minimon_config.content_order.clone();

    // Back button at top-left
    let back_button = button::text("← Back").on_press(AppMessage::Back).into();

    // Machine name with settings button on same row
    let machine_name_str = machine_name_for_title.clone();
    let settings_button = button::custom(icon::from_name("preferences-system-symbolic").size(16))
        .on_press(AppMessage::OpenMachineSensorConfig(machine_name_str));

    let title_row = row(vec![
        text(machine_name_for_title).size(18).into(),
        cosmic::widget::container(cosmic::widget::text(""))
            .width(Length::Fill)
            .into(),
        settings_button.into(),
    ])
    .align_y(Alignment::Center)
    .into();

    // Show the compact panel widget (shows what's currently visible with charts)
    // Pass sensor_config from machine_config, not full minimon_config
    let panel_widget =
        PanelWidget::view_single_machine(remote_machine, &content_order, &sensor_config);

    // Build detail sections for enabled sensors NOT visible in panel
    let mut metrics_items: Vec<Element<'static, AppMessage>> =
        vec![back_button, title_row, panel_widget];

    // CPU Load detail (shown in detail whenever NOT visible as chart in the row)
    if !cpu_chart_visible {
        let colors = ChartColors::new(DeviceKind::Cpu, ChartKind::Ring);
        let ring = RingChart::new(cpu_percent, &colors);

        metrics_items.push(
            container(
                column(vec![
                    text("CPU Load").size(16).into(),
                    row(vec![
                        canvas(ring).width(48).height(48).into(),
                        text(format!("{:.1}%", cpu_percent)).size(14).into(),
                    ])
                    .spacing(12)
                    .into(),
                ])
                .spacing(8),
            )
            .padding(16)
            .into(),
        );
    }

    // CPU Temperature detail (shown in detail whenever NOT visible as chart in the row)
    if !cpu_temp_chart_visible {
        let colors = ChartColors::new(DeviceKind::CpuTemp, ChartKind::Ring);
        let temp_percent = temp_celsius.min(100.0);
        let ring = RingChart::new_with_text(temp_percent, &format!("{:.0}", temp_celsius), &colors);

        metrics_items.push(
            container(
                column(vec![
                    text("CPU Temperature").size(16).into(),
                    row(vec![
                        canvas(ring).width(48).height(48).into(),
                        text(format!("{}°C", temp_celsius as i32)).size(14).into(),
                    ])
                    .spacing(12)
                    .into(),
                ])
                .spacing(8),
            )
            .padding(16)
            .into(),
        );
    }

    // Memory detail (shown in detail whenever NOT visible as chart in the row)
    if !memory_chart_visible {
        let colors = ChartColors::new(DeviceKind::Memory, ChartKind::Ring);
        let ring = RingChart::new(memory_percent, &colors);

        metrics_items.push(
            container(
                column(vec![
                    text("Memory Usage").size(16).into(),
                    row(vec![
                        canvas(ring).width(48).height(48).into(),
                        column(vec![
                            text(format!("{:.2} GB ({:.1}%)", memory_gb, memory_percent))
                                .size(14)
                                .into(),
                            if memory_swap_pct > 0.0 {
                                text(format!("Swap: {:.1}%", memory_swap_pct))
                                    .size(12)
                                    .into()
                            } else {
                                text("Swap: none").size(12).into()
                            },
                        ])
                        .spacing(4)
                        .into(),
                    ])
                    .spacing(12)
                    .into(),
                ])
                .spacing(8),
            )
            .padding(16)
            .into(),
        );
    }

    // Network detail (shown in detail whenever NOT visible as chart in the row)
    if !network_chart_visible {
        let rx_formatted = crate::utils::formatting::format_throughput_adaptive(network_rx_kbps);
        let tx_formatted = crate::utils::formatting::format_throughput_adaptive(network_tx_kbps);

        metrics_items.push(
            container(
                column(vec![
                    text("Network Load").size(16).into(),
                    column(vec![
                        text(format!("↓ {}", rx_formatted)).size(14).into(),
                        text(format!("↑ {}", tx_formatted)).size(14).into(),
                    ])
                    .spacing(4)
                    .into(),
                ])
                .spacing(8),
            )
            .padding(16)
            .into(),
        );
    }

    // Disk detail (disk has no chart in the row so always shows in detail)
    {
        let mut disk_items: Vec<Element<'static, AppMessage>> = vec![];

        // Header "Disk Load"
        disk_items.push(text("Disk Load").size(16).into());

        // Rate column (write/read) with adaptive unit scaling
        let write_formatted = crate::utils::formatting::format_throughput_adaptive(disk_write_kbps);
        let read_formatted = crate::utils::formatting::format_throughput_adaptive(disk_read_kbps);

        disk_items.push(
            column(vec![
                text(format!("W {}", write_formatted)).size(14).into(),
                text(format!("R {}", read_formatted)).size(14).into(),
            ])
            .spacing(4)
            .into(),
        );

        // Add partition information if any exist
        if !disk_partitions.is_empty() {
            disk_items.push(text("Partitions").size(14).into());
            for partition in disk_partitions {
                let used_gb = partition.used as f64 / 1_073_741_824.0;
                let total_gb = partition.total as f64 / 1_073_741_824.0;
                let used_percent = if partition.total > 0 {
                    (partition.used as f64 / partition.total as f64) * 100.0
                } else {
                    0.0
                };

                disk_items.push(
                    column(vec![
                        row(vec![
                            text(partition.mount.clone()).size(12).into(),
                            text(format!(
                                "{:.1}GB / {:.1}GB ({:.1}%)",
                                used_gb, total_gb, used_percent
                            ))
                            .size(12)
                            .into(),
                        ])
                        .spacing(8)
                        .into(),
                        container(
                            row(vec![
                                container(text("").size(1))
                                    .width(Length::FillPortion((used_percent * 100.0) as u16))
                                    .height(Length::Fixed(16.0))
                                    .style(move |_theme| cosmic::iced::widget::container::Style {
                                        background: Some(cosmic::iced::Background::Color(
                                            cosmic::iced::Color::from_rgb(0.8, 0.2, 0.2),
                                        )),
                                        ..Default::default()
                                    })
                                    .into(),
                                container(text("").size(1))
                                    .width(Length::FillPortion(
                                        ((100.0 - used_percent) * 100.0) as u16,
                                    ))
                                    .height(Length::Fixed(16.0))
                                    .style(move |_theme| cosmic::iced::widget::container::Style {
                                        background: Some(cosmic::iced::Background::Color(
                                            cosmic::iced::Color::from_rgb(0.3, 0.3, 0.3),
                                        )),
                                        ..Default::default()
                                    })
                                    .into(),
                            ])
                            .spacing(0),
                        )
                        .width(Length::Fill)
                        .into(),
                    ])
                    .spacing(4)
                    .into(),
                );
            }
        }

        metrics_items.push(container(column(disk_items).spacing(8)).padding(16).into());
    }

    // GPU detail sections (replace single block with three independent checks)

    // GPU Load detail
    if !gpu_load_chart_visible {
        let colors = ChartColors::new(DeviceKind::Gpu, ChartKind::Ring);
        let ring = RingChart::new(gpu_percent, &colors);

        metrics_items.push(
            container(
                column(vec![
                    text("GPU Load").size(16).into(),
                    row(vec![
                        canvas(ring).width(48).height(48).into(),
                        text(format!("{:.2}%", gpu_percent)).size(14).into(),
                    ])
                    .spacing(12)
                    .into(),
                ])
                .spacing(8),
            )
            .padding(16)
            .into(),
        );
    }

    // GPU VRAM detail
    if !gpu_vram_chart_visible {
        let colors = ChartColors::new(DeviceKind::Vram, ChartKind::Ring);
        let ring = RingChart::new(gpu_percent, &colors);

        metrics_items.push(
            container(
                column(vec![
                    text("GPU VRAM").size(16).into(),
                    row(vec![
                        canvas(ring).width(48).height(48).into(),
                        text(format!("{:.2} GB ({:.1}%)", gpu_gb, gpu_percent))
                            .size(14)
                            .into(),
                    ])
                    .spacing(12)
                    .into(),
                ])
                .spacing(8),
            )
            .padding(16)
            .into(),
        );
    }

    // GPU Temperature detail (use DeviceKind::GpuTemp)
    if !gpu_temp_chart_visible {
        let colors = ChartColors::new(DeviceKind::GpuTemp, ChartKind::Ring);
        let temp_percent = temp_celsius.min(100.0);
        let ring =
            RingChart::new_with_text(temp_percent, &format!("{}", temp_celsius as i32), &colors);

        metrics_items.push(
            container(
                column(vec![
                    text("GPU Temperature").size(16).into(),
                    row(vec![
                        canvas(ring).width(48).height(48).into(),
                        text(format!("{}°C", temp_celsius as i32)).size(14).into(),
                    ])
                    .spacing(12)
                    .into(),
                ])
                .spacing(8),
            )
            .padding(16)
            .into(),
        );
    }

    // Uptime detail (always shown if uptime_seconds > 0, no longer gated by show_uptime)
    if uptime_seconds > 0 {
        let uptime_formatted = crate::utils::formatting::format_uptime(uptime_seconds);

        metrics_items.push(
            container(
                column(vec![
                    text("Uptime").size(16).into(),
                    text(uptime_formatted).size(14).into(),
                ])
                .spacing(8),
            )
            .padding(16)
            .into(),
        );
    }

    // ── Remove machine (only for non-local machines) ─────────────────────
    if !is_local {
        let remove_name = machine_name_for_remove.clone();
        metrics_items.push(
            container(divider::horizontal::default())
                .padding([8, 0])
                .into(),
        );
        metrics_items.push(
            container(
                cosmic::widget::button::destructive("Remove this machine")
                    .on_press(AppMessage::RemoveMachine(remove_name)),
            )
            .padding([0, 16, 16, 16])
            .width(Length::Fill)
            .into(),
        );
    }

    // Wrap all content in scrollable column
    let content = column(metrics_items).spacing(16).padding([20, 16]);

    let scrollable_content = scrollable(content).height(Length::Shrink);

    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}
