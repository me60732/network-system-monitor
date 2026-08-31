//! # PanelWidget — Single-line Cosmic panel rendering respecting MinimonConfig visibility flags
//!
//! Renders desktop system stats in a single-line format suitable for the Cosmic panel.
//! Visibility of icons, charts, values, and labels is controlled by MinimonConfig settings:
//! - `config.cpu.icon_visible()` → icon display
//! - `config.cpu.chart_visible()` → ring chart display
//! - `config.cpu.value_visible()` → value text display (inside rings or standalone)
//! - `config.cpu.label_visible()` → label text display
//!
//! Layout format (single line, < 1s load target):
//! ```text
//! [cpu-icon] 0.90 [temp-icon] 34° [mem-icon] 3.00 [gpu-icon] 3.17 [net-icon] ↓ 29.4 KB/s ↑ 15.0 KB/s
//! ```
//!
//! Click-to-expand opens Main Menu with machine list.

use crate::AppMessage;
use crate::charts::{ChartColors, ChartKind, DeviceKind, RingChart};
use crate::minimon_config::{ContentOrder, ContentType, MinimonConfig};
use cosmic::widget::{button, canvas, icon, row, text};

/// Icon names (using custom minimon icons - fallback to standard if not available)
const CPU_ICON: &str = "io.github.cosmic_utils.minimon-applet-cpu";
const TEMP_ICON: &str = "io.github.cosmic_utils.minimon-applet-temperature";
const RAM_ICON: &str = "io.github.cosmic_utils.minimon-applet-ram";
const GPU_ICON: &str = "io.github.cosmic_utils.minimon-applet-gpu";
const NETWORK_ICON: &str = "io.github.cosmic_utils.minimon-applet-network";

/// PanelWidget namespace for rendering methods
pub struct PanelWidget;

impl PanelWidget {
    /// Render the panel widget from RemoteMachine instances with live data.
    pub fn view_from_machines(
        machines: &[crate::remote_machine::RemoteMachine],
        content_order: &ContentOrder,
        config: &MinimonConfig,
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
                ContentType::CpuUsage => config.cpu.chart_visible(),
                ContentType::CpuTemp => config.cputemp.chart_visible(),
                ContentType::MemoryUsage => config.memory.chart_visible(),
                ContentType::GpuInfo => config
                    .gpus
                    .get("default")
                    .map(|g| g.usage.chart_visible())
                    .unwrap_or(false),
                ContentType::NetworkUsage => config.network1.chart_visible(),
                ContentType::DiskUsage => false, // Not implemented yet
            };

            log::debug!("  {:?} - should_render={}", content_type, should_render);

            if !should_render {
                continue;
            }

            let element = match content_type {
                ContentType::CpuUsage => {
                    Self::render_cpu_with_temp(avg_cpu, avg_cpu_temp, &config.cpu, &config.cputemp)
                }
                ContentType::CpuTemp => continue, // Skip - now combined with CPU
                ContentType::MemoryUsage => {
                    Self::render_memory_metric_with_data(&avg_memory_data, &config.memory)
                }
                ContentType::GpuInfo => {
                    Self::render_gpu_group_with_data(avg_gpu_temp, &avg_gpu_data, config)
                }
                ContentType::NetworkUsage => {
                    Self::render_network_metric(rx_kbps, tx_kbps, &config.network1)
                }
                ContentType::DiskUsage => {
                    continue; // Already filtered above but keep for safety
                }
            };
            log::debug!("    → Added to metrics_items");
            metrics_items.push(element);
        }

        log::debug!("Total panel items rendered: {}", metrics_items.len());

        let metrics_row = row(metrics_items)
            .spacing(8)
            .padding([4, 8])
            .align_y(cosmic::iced::Alignment::Center);

        metrics_row.into()
    }

    /// Render a single machine's panel widget as a clickable button (for machine list)
    pub fn view_single_machine_clickable(
        machine: &crate::remote_machine::RemoteMachine,
        content_order: &ContentOrder,
        config: &MinimonConfig,
        on_click: AppMessage,
    ) -> cosmic::Element<'static, AppMessage> {
        // Extract metrics from this machine
        let cpu = machine.sensors.cpu.usage_percent;
        let cpu_temp = machine.sensors.temperature.celsius;
        let memory_data = &machine.sensors.memory;
        let gpu_temp = machine.sensors.gpu.gpu_temp.unwrap_or(0.0);
        let rx_kbps = machine.sensors.network.rx_bytes_per_sec as f64 / 1024.0;
        let tx_kbps = machine.sensors.network.tx_bytes_per_sec as f64 / 1024.0;

        // Build metrics row dynamically based on content order and visibility flags
        let mut metrics_items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        for content_type in &content_order.order {
            // Check if sensor is enabled before rendering
            let should_render = match content_type {
                ContentType::CpuUsage => config.cpu.chart_visible(),
                ContentType::CpuTemp => config.cputemp.chart_visible(),
                ContentType::MemoryUsage => config.memory.chart_visible(),
                ContentType::GpuInfo => config
                    .gpus
                    .get("default")
                    .map(|g| g.usage.chart_visible())
                    .unwrap_or(false),
                ContentType::NetworkUsage => config.network1.chart_visible(),
                ContentType::DiskUsage => false, // Not implemented yet
            };

            if !should_render {
                continue;
            }

            let element = match content_type {
                ContentType::CpuUsage => {
                    Self::render_cpu_with_temp(cpu, cpu_temp, &config.cpu, &config.cputemp)
                }
                ContentType::CpuTemp => continue, // handled inside render_cpu_with_temp
                ContentType::MemoryUsage => {
                    Self::render_memory_metric_with_data(memory_data, &config.memory)
                }
                ContentType::GpuInfo => {
                    Self::render_gpu_group_with_data(gpu_temp, &machine.sensors.gpu, config)
                }
                ContentType::NetworkUsage => {
                    Self::render_network_metric(rx_kbps, tx_kbps, &config.network1)
                }
                ContentType::DiskUsage => {
                    continue; // Already filtered above but keep for safety
                }
            };
            metrics_items.push(element);
        }

        let metrics_row = row(metrics_items)
            .spacing(8)
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
        config: &MinimonConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        // Extract metrics from this machine
        let cpu = machine.sensors.cpu.usage_percent;
        let cpu_temp = machine.sensors.temperature.celsius;
        let memory_data = &machine.sensors.memory;
        let gpu_temp = machine.sensors.gpu.gpu_temp.unwrap_or(0.0);
        let rx_kbps = machine.sensors.network.rx_bytes_per_sec as f64 / 1024.0;
        let tx_kbps = machine.sensors.network.tx_bytes_per_sec as f64 / 1024.0;

        // Build metrics row dynamically based on content order and visibility flags
        let mut metrics_items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        for content_type in &content_order.order {
            // Check if sensor is enabled before rendering
            let should_render = match content_type {
                ContentType::CpuUsage => config.cpu.chart_visible(),
                ContentType::CpuTemp => config.cputemp.chart_visible(),
                ContentType::MemoryUsage => config.memory.chart_visible(),
                ContentType::GpuInfo => config
                    .gpus
                    .get("default")
                    .map(|g| g.usage.chart_visible())
                    .unwrap_or(false),
                ContentType::NetworkUsage => config.network1.chart_visible(),
                ContentType::DiskUsage => false, // Not implemented yet
            };

            if !should_render {
                continue;
            }

            let element = match content_type {
                ContentType::CpuUsage => {
                    Self::render_cpu_with_temp(cpu, cpu_temp, &config.cpu, &config.cputemp)
                }
                ContentType::CpuTemp => continue, // Skip - now combined with CPU
                ContentType::MemoryUsage => {
                    Self::render_memory_metric_with_data(memory_data, &config.memory)
                }
                ContentType::GpuInfo => {
                    Self::render_gpu_group_with_data(gpu_temp, &machine.sensors.gpu, config)
                }
                ContentType::NetworkUsage => {
                    Self::render_network_metric(rx_kbps, tx_kbps, &config.network1)
                }
                ContentType::DiskUsage => {
                    continue; // Already filtered above but keep for safety
                }
            };
            metrics_items.push(element);
        }

        row(metrics_items)
            .spacing(8)
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
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        // Show icon if either config wants it
        if cpu_config.icon_visible() {
            items.push(icon::from_name(CPU_ICON).symbolic(true).size(20).into());
        }

        // Show label if either config wants it
        if cpu_config.label_visible() {
            items.push(text("CPU").size(11).into());
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
            let temp_ring =
                RingChart::new_with_text(temp_percent, &format!("{}°", temp as i32), &temp_colors);
            items.push(canvas(temp_ring).width(36).height(36).into());
        }

        row(items)
            .spacing(4)
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
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        if config.icon_visible() {
            items.push(icon::from_name(RAM_ICON).symbolic(true).size(20).into());
        }

        if config.label_visible() {
            items.push(text("MEM").size(11).into());
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
            let ring =
                RingChart::new_with_text(data.usage_percent(), &Self::fmt_ring(used_gb), &colors);
            items.push(canvas(ring).width(36).height(36).into());
        }

        row(items)
            .spacing(4)
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    /// Render memory metric with icon + ring (value centered inside) - DEPRECATED, use render_memory_metric_with_data
    #[allow(dead_code)]
    fn render_memory_metric(
        value: f32,
        config: &crate::minimon_config::MemoryConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        if config.icon_visible() {
            items.push(icon::from_name(RAM_ICON).symbolic(true).size(20).into());
        }

        if config.label_visible() {
            items.push(text("MEM").size(11).into());
        }

        // Always render chart when sensor is shown
        let colors = ChartColors::new(DeviceKind::Memory, ChartKind::Ring);
        let ring = RingChart::new(value, &colors);
        items.push(canvas(ring).width(36).height(36).into());

        row(items)
            .spacing(4)
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    /// Render GPU group with data object (shows GB used for VRAM, not percentage)
    fn render_gpu_group_with_data(
        temp: f32,
        gpu_data: &crate::simple_sensors::GpuData,
        config: &MinimonConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        // Get GPU config from HashMap, use defaults if not present
        let gpu_config = config.gpus.get("default");

        let show_icon = gpu_config.map(|g| g.usage.icon_visible()).unwrap_or(true);
        let show_label = gpu_config.map(|g| g.usage.label_visible()).unwrap_or(false);
        let show_temp = gpu_config.map(|g| g.temp.chart_visible()).unwrap_or(false);
        let show_load = gpu_config.map(|g| g.usage.chart_visible()).unwrap_or(true);
        let show_vram = gpu_config.map(|g| g.vram.chart_visible()).unwrap_or(false);

        if show_icon {
            items.push(icon::from_name(GPU_ICON).symbolic(true).size(20).into());
        }

        if show_label {
            items.push(text("GPU").size(11).into());
        }

        // Render temp chart if enabled
        if show_temp {
            let temp_percent = (temp / 100.0).clamp(0.0, 1.0) * 100.0;
            let temp_colors = ChartColors::new(DeviceKind::GpuTemp, ChartKind::Ring);
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
            let show_as_percentage = gpu_config.map(|g| g.vram.percentage).unwrap_or(false);

            if show_as_percentage {
                // Display percentage text inside ring
                let vram_ring = RingChart::new(vram_percent, &vram_colors);
                items.push(canvas(vram_ring).width(36).height(36).into());
            } else {
                // Display GB used as text inside ring (not percentage)
                let vram_gb = gpu_data.vram_used_bytes as f64 / 1_073_741_824.0;
                let vram_ring =
                    RingChart::new_with_text(vram_percent, &Self::fmt_ring(vram_gb), &vram_colors);
                items.push(canvas(vram_ring).width(36).height(36).into());
            }
        }

        row(items)
            .spacing(4)
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    /// Render GPU group (DEPRECATED - use render_gpu_group_with_data for correct VRAM display)
    #[allow(dead_code)]
    fn render_gpu_group(
        temp: f32,
        load: f32,
        vram: f32,
        config: &MinimonConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        // Get GPU config from HashMap, use defaults if not present
        let gpu_config = config.gpus.get("default");

        let show_icon = gpu_config.map(|g| g.usage.icon_visible()).unwrap_or(true);
        let show_label = gpu_config.map(|g| g.usage.label_visible()).unwrap_or(false);
        let show_temp = gpu_config.map(|g| g.temp.chart_visible()).unwrap_or(false);
        let show_load = gpu_config.map(|g| g.usage.chart_visible()).unwrap_or(true);
        let show_vram = gpu_config.map(|g| g.vram.chart_visible()).unwrap_or(false);

        if show_icon {
            items.push(icon::from_name(GPU_ICON).symbolic(true).size(20).into());
        }

        if show_label {
            items.push(text("GPU").size(11).into());
        }

        // Render temp chart if enabled
        if show_temp {
            let temp_percent = (temp / 100.0).clamp(0.0, 1.0) * 100.0;
            let temp_colors = ChartColors::new(DeviceKind::GpuTemp, ChartKind::Ring);
            let temp_ring =
                RingChart::new_with_text(temp_percent, &format!("{}°", temp as i32), &temp_colors);
            items.push(canvas(temp_ring).width(36).height(36).into());
        }

        // Render load chart if enabled
        if show_load {
            let load_colors = ChartColors::new(DeviceKind::Gpu, ChartKind::Ring);
            let load_ring = RingChart::new(load, &load_colors);
            items.push(canvas(load_ring).width(36).height(36).into());
        }

        // Render VRAM chart if enabled
        if show_vram {
            let vram_colors = ChartColors::new(DeviceKind::Vram, ChartKind::Ring);
            let vram_ring = RingChart::new(vram, &vram_colors);
            items.push(canvas(vram_ring).width(36).height(36).into());
        }

        row(items)
            .spacing(4)
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    /// Render network metrics - only text (no chart)
    fn render_network_metric(
        rx_kbps: f64,
        tx_kbps: f64,
        config: &crate::minimon_config::NetworkConfig,
    ) -> cosmic::Element<'static, AppMessage> {
        let mut items: Vec<cosmic::Element<'static, AppMessage>> = Vec::new();

        if config.icon_visible() {
            items.push(icon::from_name(NETWORK_ICON).symbolic(true).size(20).into());
        }

        if config.label_visible() {
            items.push(text("NET").size(11).into());
        }

        // Only show the text speeds with adaptive unit scaling
        use cosmic::widget::column;
        let rx_formatted = crate::utils::formatting::format_throughput_adaptive(rx_kbps);
        let tx_formatted = crate::utils::formatting::format_throughput_adaptive(tx_kbps);

        items.push(
            column(vec![
                text(format!("↓{}", rx_formatted)).size(9).into(),
                text(format!("↑{}", tx_formatted)).size(9).into(),
            ])
            .spacing(1)
            .into(),
        );

        row(items)
            .spacing(4)
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }
}
