use crate::AppMessage;
use crate::charts::ring::RingChart;
use crate::minimon_config::{ChartColors, ChartKind, ContentOrder, ContentType, DeviceKind};
use cosmic::widget::{button, canvas, icon, row, text};

const CPU_ICON: &str = "io.github.cosmic_utils.minimon-applet-cpu";
const TEMP_ICON: &str = "io.github.cosmic_utils.minimon-applet-temperature";
const RAM_ICON: &str = "io.github.cosmic_utils.minimon-applet-ram";
const GPU_ICON: &str = "io.github.cosmic_utils.minimon-applet-gpu";
const NETWORK_ICON: &str = "io.github.cosmic_utils.minimon-applet-network";
const DISK_ICON: &str = "drive-harddisk-symbolic";

/// Global display settings threaded into all render helpers.
#[derive(Clone, Copy)]
pub struct GlobalDisplayConfig {
    pub value_size: u16,
    pub monospace: bool,
    pub spacing: u16,
}

impl GlobalDisplayConfig {
    pub fn from_minimon(config: &crate::minimon_config::MinimonConfig) -> Self {
        Self {
            value_size: config.value_size_default,
            monospace: config.monospace_values,
            spacing: config.panel_spacing,
        }
    }
}

/// PanelWidget namespace for rendering methods
pub struct PanelWidget;

impl PanelWidget {
    /// Render the panel widget from RemoteMachine instances with live data.
    pub fn view_from_machines(
        machines: &[crate::remote_machine::RemoteMachine],
        content_order: &ContentOrder,
        sensor_config: &crate::minimon_config::MachineSensorConfig,
        display: &GlobalDisplayConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        // Aggregate metrics from all machines (simple average for now)
        let mut total_cpu = 0.0;
        let mut total_cpu_temp = 0.0;
        let mut total_gpu = 0.0;
        let mut total_gpu_temp = 0.0;
        let mut total_rx = 0.0;
        let mut total_tx = 0.0;
        let count = machines.len().max(1) as f32;

        for machine in machines {
            total_cpu += machine.sensors.cpu.usage_percent;
            total_cpu_temp += machine.sensors.temperature.celsius;
            // GPU load from load_percent field (actual GPU utilization)
            total_gpu += machine.sensors.gpu.load_percent.unwrap_or(0.0);
            // GPU temp from GPU sensor's own temp field (separate from CPU temp)
            total_gpu_temp += machine.sensors.gpu.gpu_temp.unwrap_or(0.0);
            total_rx += machine.sensors.network.rx_bytes_per_sec as f64;
            total_tx += machine.sensors.network.tx_bytes_per_sec as f64;
        }

        let avg_cpu = total_cpu / count;
        let avg_cpu_temp = total_cpu_temp / count;

        // Calculate average memory data for bytes display
        let mut total_memory_used = 0u64;
        let mut total_memory_total = 0u64;
        for machine in machines {
            total_memory_used += machine.sensors.memory.used_bytes;
            total_memory_total += machine.sensors.memory.total_bytes;
        }
        let avg_memory_data = crate::simple_sensors::MemoryData {
            used_bytes: total_memory_used / machines.len().max(1) as u64,
            total_bytes: total_memory_total / machines.len().max(1) as u64,
            swap_used_pct: 0.0, // panel bar shows ring only, no swap display
        };

        let avg_gpu = total_gpu / count;
        let avg_gpu_temp = total_gpu_temp / count;
        let rx_kbps = total_rx / 1024.0;
        let tx_kbps = total_tx / 1024.0;

        // Calculate average disk data
        let mut total_disk_write = 0.0f64;
        let mut total_disk_read = 0.0f64;
        for machine in machines {
            total_disk_write += machine.sensors.disk.write_bytes_per_sec as f64;
            total_disk_read += machine.sensors.disk.read_bytes_per_sec as f64;
        }
        let avg_disk_write_kbps = total_disk_write / 1024.0;
        let avg_disk_read_kbps = total_disk_read / 1024.0;

        // Calculate average GPU data for VRAM bytes display
        let mut total_vram_used = 0u64;
        let mut total_vram_total = 0u64;
        for machine in machines {
            total_vram_used += machine.sensors.gpu.vram_used_bytes;
            total_vram_total += machine.sensors.gpu.vram_total_bytes;
        }
        let avg_gpu_data = crate::simple_sensors::GpuData {
            vram_used_bytes: total_vram_used / machines.len().max(1) as u64,
            vram_total_bytes: total_vram_total / machines.len().max(1) as u64,
            load_percent: Some(avg_gpu),
            gpu_temp: Some(avg_gpu_temp),
        };

        // Build metrics row dynamically based on content order and visibility flags
        let mut metrics_items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        log::debug!(
            "Panel Widget Rendering: {} machines, {} content items",
            machines.len(),
            content_order.order.len()
        );

        for content_type in &content_order.order {
            // Check if sensor is enabled before rendering
            let should_render = match content_type {
                ContentType::CpuUsage => sensor_config.cpu.chart_visible(),
                ContentType::CpuTemp => sensor_config.cputemp.chart_visible(),
                ContentType::MemoryUsage => sensor_config.memory.chart_visible(),
                ContentType::GpuInfo => sensor_config.gpu.usage.chart_visible(),
                ContentType::NetworkUsage => sensor_config.network1.chart_visible(),
                ContentType::DiskUsage => sensor_config.disks1.chart_visible(),
            };

            log::debug!("  {:?} - should_render={}", content_type, should_render);

            if !should_render {
                continue;
            }

            let element = match content_type {
                ContentType::CpuUsage => Self::render_cpu_with_temp(
                    avg_cpu,
                    avg_cpu_temp,
                    &sensor_config.cpu,
                    &sensor_config.cputemp,
                    display,
                ),
                ContentType::CpuTemp => continue, // Skip - now combined with CPU
                ContentType::MemoryUsage => Self::render_memory_metric_with_data(
                    &avg_memory_data,
                    &sensor_config.memory,
                    display,
                ),
                ContentType::GpuInfo => Self::render_gpu_group_with_data(
                    avg_gpu_temp,
                    &avg_gpu_data,
                    &sensor_config.gpu,
                    display,
                ),
                ContentType::NetworkUsage => {
                    Self::render_network_metric(rx_kbps, tx_kbps, &sensor_config.network1, display)
                }
                ContentType::DiskUsage => Self::render_disk_metric(
                    avg_disk_write_kbps,
                    avg_disk_read_kbps,
                    &sensor_config.disks1,
                    display,
                ),
            };
            log::debug!("    → Added to metrics_items");
            metrics_items.push(element);
        }

        log::debug!("Total panel items rendered: {}", metrics_items.len());

        let metrics_row = row(metrics_items)
            .spacing(2 + display.spacing * 2)
            .padding([4, 8])
            .align_y(cosmic::iced::Alignment::Center);

        metrics_row.into()
    }

    /// Render a single machine's panel widget as a clickable button (for machine list)
    pub fn view_single_machine_clickable(
        machine: &crate::remote_machine::RemoteMachine,
        content_order: &ContentOrder,
        sensor_config: &crate::minimon_config::MachineSensorConfig,
        on_click: AppMessage,
        display: &GlobalDisplayConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        // Extract metrics from this machine
        let cpu = machine.sensors.cpu.usage_percent;
        let cpu_temp = machine.sensors.temperature.celsius;
        let memory_data = &machine.sensors.memory;
        let gpu_temp = machine.sensors.gpu.gpu_temp.unwrap_or(0.0);
        let rx_kbps = machine.sensors.network.rx_bytes_per_sec as f64 / 1024.0;
        let tx_kbps = machine.sensors.network.tx_bytes_per_sec as f64 / 1024.0;
        let disk_write_kbps = machine.sensors.disk.write_bytes_per_sec as f64 / 1024.0;
        let disk_read_kbps = machine.sensors.disk.read_bytes_per_sec as f64 / 1024.0;

        // Build metrics row dynamically based on content order and visibility flags
        let mut metrics_items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        for content_type in &content_order.order {
            // Check if sensor is enabled before rendering
            let should_render = match content_type {
                ContentType::CpuUsage => sensor_config.cpu.chart_visible(),
                ContentType::CpuTemp => sensor_config.cputemp.chart_visible(),
                ContentType::MemoryUsage => sensor_config.memory.chart_visible(),
                ContentType::GpuInfo => sensor_config.gpu.usage.chart_visible(),
                ContentType::NetworkUsage => sensor_config.network1.chart_visible(),
                ContentType::DiskUsage => sensor_config.disks1.chart_visible(),
            };

            if !should_render {
                continue;
            }

            let element = match content_type {
                ContentType::CpuUsage => Self::render_cpu_with_temp(
                    cpu,
                    cpu_temp,
                    &sensor_config.cpu,
                    &sensor_config.cputemp,
                    display,
                ),
                ContentType::CpuTemp => continue, // handled inside render_cpu_with_temp
                ContentType::MemoryUsage => Self::render_memory_metric_with_data(
                    memory_data,
                    &sensor_config.memory,
                    display,
                ),
                ContentType::GpuInfo => Self::render_gpu_group_with_data(
                    gpu_temp,
                    &machine.sensors.gpu,
                    &sensor_config.gpu,
                    display,
                ),
                ContentType::NetworkUsage => {
                    Self::render_network_metric(rx_kbps, tx_kbps, &sensor_config.network1, display)
                }
                ContentType::DiskUsage => Self::render_disk_metric(
                    disk_write_kbps,
                    disk_read_kbps,
                    &sensor_config.disks1,
                    display,
                ),
            };
            metrics_items.push(element);
        }

        let metrics_row = row(metrics_items)
            .spacing(2 + display.spacing * 2)
            .padding([4, 8])
            .align_y(cosmic::iced::Alignment::Center);

        // Wrap in button with AppletIcon style (same as main panel - hover-only background)
        button::custom(metrics_row)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press(on_click)
            .into()
    }

    /// Render a single machine's panel widget (not clickable - used in machine list)
    pub fn view_single_machine(
        machine: &crate::remote_machine::RemoteMachine,
        content_order: &ContentOrder,
        sensor_config: &crate::minimon_config::MachineSensorConfig,
        display: &GlobalDisplayConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        // Extract metrics from this machine
        let cpu = machine.sensors.cpu.usage_percent;
        let cpu_temp = machine.sensors.temperature.celsius;
        let memory_data = &machine.sensors.memory;
        let gpu_temp = machine.sensors.gpu.gpu_temp.unwrap_or(0.0);
        let rx_kbps = machine.sensors.network.rx_bytes_per_sec as f64 / 1024.0;
        let tx_kbps = machine.sensors.network.tx_bytes_per_sec as f64 / 1024.0;
        let disk_write_kbps = machine.sensors.disk.write_bytes_per_sec as f64 / 1024.0;
        let disk_read_kbps = machine.sensors.disk.read_bytes_per_sec as f64 / 1024.0;

        // Build metrics row dynamically based on content order and visibility flags
        let mut metrics_items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        for content_type in &content_order.order {
            // Check if sensor is enabled before rendering
            let should_render = match content_type {
                ContentType::CpuUsage => sensor_config.cpu.chart_visible(),
                ContentType::CpuTemp => sensor_config.cputemp.chart_visible(),
                ContentType::MemoryUsage => sensor_config.memory.chart_visible(),
                ContentType::GpuInfo => sensor_config.gpu.usage.chart_visible(),
                ContentType::NetworkUsage => sensor_config.network1.chart_visible(),
                ContentType::DiskUsage => sensor_config.disks1.chart_visible(),
            };

            if !should_render {
                continue;
            }

            let element = match content_type {
                ContentType::CpuUsage => Self::render_cpu_with_temp(
                    cpu,
                    cpu_temp,
                    &sensor_config.cpu,
                    &sensor_config.cputemp,
                    display,
                ),
                ContentType::CpuTemp => continue, // Skip - now combined with CPU
                ContentType::MemoryUsage => Self::render_memory_metric_with_data(
                    memory_data,
                    &sensor_config.memory,
                    display,
                ),
                ContentType::GpuInfo => Self::render_gpu_group_with_data(
                    gpu_temp,
                    &machine.sensors.gpu,
                    &sensor_config.gpu,
                    display,
                ),
                ContentType::NetworkUsage => {
                    Self::render_network_metric(rx_kbps, tx_kbps, &sensor_config.network1, display)
                }
                ContentType::DiskUsage => Self::render_disk_metric(
                    disk_write_kbps,
                    disk_read_kbps,
                    &sensor_config.disks1,
                    display,
                ),
            };
            metrics_items.push(element);
        }

        row(metrics_items)
            .spacing(2 + display.spacing * 2)
            .padding([4, 8])
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    /// Render CPU metric with temperature (combined panel)
    fn render_cpu_with_temp(
        cpu_percent: f32,
        temp: f32,
        cpu_config: &crate::minimon_config::CpuConfig,
        temp_config: &crate::minimon_config::CpuTempConfig,
        display: &GlobalDisplayConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        // Show icon if either config wants it
        if cpu_config.icon_visible() {
            items.push(icon::from_name(CPU_ICON).symbolic(true).size(20).into());
        }

        // Show label if either config wants it
        if cpu_config.label_visible() {
            let mut cpu_text = text("CPU").size(display.value_size);
            if display.monospace {
                cpu_text = cpu_text.font(cosmic::iced::Font::MONOSPACE);
            }
            items.push(cpu_text.into());
        }

        // Always render CPU usage chart
        let cpu_colors = ChartColors::new(DeviceKind::Cpu, ChartKind::Ring);
        let cpu_ring = RingChart::new(cpu_percent, &cpu_colors);
        items.push(canvas(cpu_ring).width(36).height(36).into());

        // Render temp chart + icon if enabled in temp config
        if temp_config.chart_visible() {
            if temp_config.icon_visible() {
                items.push(icon::from_name(TEMP_ICON).symbolic(true).size(20).into());
            }
            let temp_percent = (temp / 100.0).clamp(0.0, 1.0) * 100.0;
            let temp_colors = ChartColors::new(DeviceKind::CpuTemp, ChartKind::Ring);
            // Note: font override not applied to ring text (canvas-rendered)
            let temp_ring =
                RingChart::new_with_text(temp_percent, &format!("{}°", temp as i32), &temp_colors);
            items.push(canvas(temp_ring).width(36).height(36).into());
        }

        row(items)
            .spacing(display.spacing.max(2))
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    /// Format a float value for display inside a ring chart.
    /// Keeps text to ≤ 4 characters (3 significant + optional decimal point).
    fn fmt_ring(value: f64) -> String {
        if value < 10.0 {
            format!("{:.2}", value) // "9.99" (4 chars)
        } else if value < 100.0 {
            format!("{:.1}", value) // "24.5" (4 chars)
        } else {
            format!("{:.0}", value) // "100"  (3 chars)
        }
    }

    /// Render memory metric with icon + ring (value centered inside) or bytes as text
    fn render_memory_metric_with_data(
        data: &crate::simple_sensors::MemoryData,
        config: &crate::minimon_config::MemoryConfig,
        display: &GlobalDisplayConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        if config.icon_visible() {
            items.push(icon::from_name(RAM_ICON).symbolic(true).size(20).into());
        }

        if config.label_visible() {
            let mut mem_text = text("MEM").size(display.value_size);
            if display.monospace {
                mem_text = mem_text.font(cosmic::iced::Font::MONOSPACE);
            }
            items.push(mem_text.into());
        }

        // Check percentage config to determine text format
        if config.percentage {
            // Display percentage text inside ring
            let colors = ChartColors::new(DeviceKind::Memory, ChartKind::Ring);
            let ring = RingChart::new(data.usage_percent(), &colors);
            items.push(canvas(ring).width(36).height(36).into());
        } else {
            // Display GB used as text inside ring (not percentage)
            let used_gb = data.used_bytes as f64 / 1_073_741_824.0;
            let colors = ChartColors::new(DeviceKind::Memory, ChartKind::Ring);
            // Note: font override not applied to ring text (canvas-rendered)
            let ring =
                RingChart::new_with_text(data.usage_percent(), &Self::fmt_ring(used_gb), &colors);
            items.push(canvas(ring).width(36).height(36).into());
        }

        row(items)
            .spacing(display.spacing.max(2))
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    /// Render memory metric with icon + ring (value centered inside) - DEPRECATED, use render_memory_metric_with_data
    #[allow(dead_code)]
    fn render_memory_metric(
        value: f32,
        config: &crate::minimon_config::MemoryConfig,
        display: &GlobalDisplayConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        if config.icon_visible() {
            items.push(icon::from_name(RAM_ICON).symbolic(true).size(20).into());
        }

        if config.label_visible() {
            let mut mem_text = text("MEM").size(display.value_size);
            if display.monospace {
                mem_text = mem_text.font(cosmic::iced::Font::MONOSPACE);
            }
            items.push(mem_text.into());
        }

        // Always render chart when sensor is shown
        let colors = ChartColors::new(DeviceKind::Memory, ChartKind::Ring);
        let ring = RingChart::new(value, &colors);
        items.push(canvas(ring).width(36).height(36).into());

        row(items)
            .spacing(display.spacing.max(2))
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    /// Render GPU group with data object (shows GB used for VRAM, not percentage)
    fn render_gpu_group_with_data(
        temp: f32,
        gpu_data: &crate::simple_sensors::GpuData,
        gpu_config: &crate::minimon_config::GpuConfig,
        display: &GlobalDisplayConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        let show_icon = gpu_config.usage.icon_visible();
        let show_label = gpu_config.usage.label_visible();
        let show_temp = gpu_config.temp.chart_visible();
        let show_load = gpu_config.usage.chart_visible();
        let show_vram = gpu_config.vram.chart_visible();

        if show_icon {
            items.push(icon::from_name(GPU_ICON).symbolic(true).size(20).into());
        }

        if show_label {
            let mut gpu_text = text("GPU").size(display.value_size);
            if display.monospace {
                gpu_text = gpu_text.font(cosmic::iced::Font::MONOSPACE);
            }
            items.push(gpu_text.into());
        }

        // Render temp chart if enabled
        if show_temp {
            let temp_percent = (temp / 100.0).clamp(0.0, 1.0) * 100.0;
            let temp_colors = ChartColors::new(DeviceKind::GpuTemp, ChartKind::Ring);
            // Note: font override not applied to ring text (canvas-rendered)
            let temp_ring =
                RingChart::new_with_text(temp_percent, &format!("{}°", temp as i32), &temp_colors);
            items.push(canvas(temp_ring).width(36).height(36).into());
        }

        // Render load chart if enabled (percentage)
        if show_load {
            if let Some(load) = gpu_data.load_percent {
                let load_colors = ChartColors::new(DeviceKind::Gpu, ChartKind::Ring);
                let load_ring = RingChart::new(load, &load_colors);
                items.push(canvas(load_ring).width(36).height(36).into());
            }
        }

        // Render VRAM chart if enabled (check percentage config)
        if show_vram {
            let vram_percent = gpu_data.usage_percent();
            let vram_colors = ChartColors::new(DeviceKind::Vram, ChartKind::Ring);

            // Check if percentage display is enabled (like memory)
            let show_as_percentage = gpu_config.vram.percentage;

            if show_as_percentage {
                // Display percentage text inside ring
                let vram_ring = RingChart::new(vram_percent, &vram_colors);
                items.push(canvas(vram_ring).width(36).height(36).into());
            } else {
                // Display GB used as text inside ring (not percentage)
                let vram_gb = gpu_data.vram_used_bytes as f64 / 1_073_741_824.0;
                // Note: font override not applied to ring text (canvas-rendered)
                let vram_ring =
                    RingChart::new_with_text(vram_percent, &Self::fmt_ring(vram_gb), &vram_colors);
                items.push(canvas(vram_ring).width(36).height(36).into());
            }
        }

        row(items)
            .spacing(display.spacing.max(2))
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    /// Render network metrics - only text (no chart)
    fn render_network_metric(
        rx_kbps: f64,
        tx_kbps: f64,
        config: &crate::minimon_config::NetworkConfig,
        display: &GlobalDisplayConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        if config.icon_visible() {
            items.push(icon::from_name(NETWORK_ICON).symbolic(true).size(20).into());
        }

        if config.label_visible() {
            let mut net_text = text("NET").size(display.value_size);
            if display.monospace {
                net_text = net_text.font(cosmic::iced::Font::MONOSPACE);
            }
            items.push(net_text.into());
        }

        use cosmic::widget::column;
        let rx_formatted = crate::utils::formatting::format_throughput_adaptive(rx_kbps);
        let tx_formatted = crate::utils::formatting::format_throughput_adaptive(tx_kbps);

        items.push(
            column(vec![
                {
                    let mut rx_text = text(format!("↓{}", rx_formatted))
                        .size(display.value_size.saturating_sub(2).max(7));
                    if display.monospace {
                        rx_text = rx_text.font(cosmic::iced::Font::MONOSPACE);
                    }
                    rx_text.into()
                },
                {
                    let mut tx_text = text(format!("↑{}", tx_formatted))
                        .size(display.value_size.saturating_sub(2).max(7));
                    if display.monospace {
                        tx_text = tx_text.font(cosmic::iced::Font::MONOSPACE);
                    }
                    tx_text.into()
                },
            ])
            .spacing(1)
            .into(),
        );

        row(items)
            .spacing(display.spacing.max(2))
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    /// Render disk metrics — icon + stacked W/R throughput text (mirrors network layout)
    fn render_disk_metric(
        write_kbps: f64,
        read_kbps: f64,
        config: &crate::minimon_config::DisksConfig,
        display: &GlobalDisplayConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        if config.icon_visible() {
            items.push(icon::from_name(DISK_ICON).symbolic(true).size(20).into());
        }

        if config.label_visible() {
            let mut dsk_text = text("DSK").size(display.value_size);
            if display.monospace {
                dsk_text = dsk_text.font(cosmic::iced::Font::MONOSPACE);
            }
            items.push(dsk_text.into());
        }

        use cosmic::widget::column;
        let write_formatted = crate::utils::formatting::format_throughput_adaptive(write_kbps);
        let read_formatted = crate::utils::formatting::format_throughput_adaptive(read_kbps);

        items.push(
            column(vec![
                {
                    let mut write_text = text(format!("W{}", write_formatted))
                        .size(display.value_size.saturating_sub(2).max(7));
                    if display.monospace {
                        write_text = write_text.font(cosmic::iced::Font::MONOSPACE);
                    }
                    write_text.into()
                },
                {
                    let mut read_text = text(format!("R{}", read_formatted))
                        .size(display.value_size.saturating_sub(2).max(7));
                    if display.monospace {
                        read_text = read_text.font(cosmic::iced::Font::MONOSPACE);
                    }
                    read_text.into()
                },
            ])
            .spacing(1)
            .into(),
        );

        row(items)
            .spacing(display.spacing.max(2))
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }
}
