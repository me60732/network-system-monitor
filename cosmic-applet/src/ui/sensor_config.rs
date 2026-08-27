//! # Sensor Configuration Views
//!
//! Individual configuration panels for each sensor type (CPU, Memory, Network, Disk, GPU, Temperature).
//! Each sensor has detailed options for chart display, labels, icons, and sensor-specific settings.
//! All charts use Ring type only (no chart type dropdown needed).

use crate::{AppMessage, minimon_config::MinimonConfig};
use cosmic::widget::{button, column, container, divider, row, scrollable, text, toggler};
use cosmic::iced::{Alignment, Length};
use cosmic::Element;
use crate::minimon_config::{NetworkVariant, DisksVariant};

/// View for CPU sensor configuration (includes load and temperature)
pub fn view_cpu_config(config: &MinimonConfig) -> Element<'static, AppMessage> {
    let mut items: Vec<Element<AppMessage>> = vec![
        button::text("← Back")
            .on_press(AppMessage::Back)
            .into(),
        text("CPU").size(24).into(),
    ];

    // === CPU Load Section ===
    items.push(text("Load Average").size(16).into());

    // Show sensor
    items.push(
        row(vec![
            text("Show sensor").into(),
            toggler(config.cpu.chart_visible())
                .on_toggle(AppMessage::ToggleCpuShowChart)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show label
    items.push(
        row(vec![
            text("Show label").into(),
            toggler(config.cpu.label_visible())
                .on_toggle(AppMessage::ToggleCpuShowLabel)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show icon
    items.push(
        row(vec![
            text("Show icon").into(),
            toggler(config.cpu.icon_visible())
                .on_toggle(AppMessage::ToggleCpuShowIcon)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Colors button
    items.push(
        container(
            button::text("Colors")
        )
        .width(Length::Fill)
        .into()
    );

    // Divider between sections
    items.push(divider::horizontal::default().into());

    // === CPU Temperature Section ===
    items.push(text("Temperature").size(16).into());
    items.push(
        text("For Intel processors shows single highest temperature found across all sensors/cores.")
            .size(12)
            .into()
    );

    // Show sensor
    items.push(
        row(vec![
            text("Show sensor").into(),
            toggler(config.cputemp.chart_visible())
                .on_toggle(AppMessage::ToggleCpuTempShowChart)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show label
    items.push(
        row(vec![
            text("Show label").into(),
            toggler(config.cputemp.label_visible())
                .on_toggle(AppMessage::ToggleCpuTempShowLabel)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show icon
    items.push(
        row(vec![
            text("Show icon").into(),
            toggler(config.cputemp.icon_visible())
                .on_toggle(AppMessage::ToggleCpuTempShowIcon)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    let content = column(items)
        .spacing(16)
        .padding(24);
    
    let scrollable_content = scrollable(content)
        .height(Length::Shrink);
    
    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}

/// View for CPU Temperature sensor configuration
pub fn view_cpu_temp_config(config: &MinimonConfig) -> Element<'static, AppMessage> {
    let mut items: Vec<Element<AppMessage>> = vec![
        button::text("← Back")
            .on_press(AppMessage::Back)
            .into(),
        text("CPU Temperature").size(24).into(),
        text("For Intel processors shows single highest temperature found across all sensors/cores.")
            .size(12)
            .into(),
    ];

    // Show sensor
    items.push(
        row(vec![
            text("Show sensor").into(),
            toggler(config.cputemp.chart_visible())
                .on_toggle(AppMessage::ToggleCpuTempShowChart)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show label
    items.push(
        row(vec![
            text("Show label").into(),
            toggler(config.cputemp.label_visible())
                .on_toggle(AppMessage::ToggleCpuTempShowLabel)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show icon
    items.push(
        row(vec![
            text("Show icon").into(),
            toggler(config.cputemp.icon_visible())
                .on_toggle(AppMessage::ToggleCpuTempShowIcon)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Temperature unit dropdown
    items.push(
        row(vec![
            text("Temperature unit").into(),
            button::text("Celsius ▾")
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Minimum temperature input
    items.push(
        row(vec![
            text("Minimum temperature").into(),
            button::text("0")
                .width(Length::Fixed(80.0))
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Colors button
    items.push(
        container(
            button::text("Colors")
        )
        .width(Length::Fill)
        .into()
    );

    let content = column(items)
        .spacing(16)
        .padding(24);
    
    let scrollable_content = scrollable(content)
        .height(Length::Shrink);
    
    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}

/// View for Memory sensor configuration
pub fn view_memory_config(config: &MinimonConfig) -> Element<'static, AppMessage> {
    let mut items: Vec<Element<AppMessage>> = vec![
        button::text("← Back")
            .on_press(AppMessage::Back)
            .into(),
        text("Memory Usage").size(24).into(),
    ];

    // Show sensor
    items.push(
        row(vec![
            text("Show sensor").into(),
            toggler(config.memory.chart_visible())
                .on_toggle(AppMessage::ToggleMemoryShowChart)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show allocated on chart
    items.push(
        row(vec![
            text("Show allocated on chart").into(),
            toggler(config.memory.show_allocated)
                .on_toggle(AppMessage::ToggleMemoryShowAllocated)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Tooltip text
    items.push(
        text("Allocated = total minus free. Includes system cache and buffers, which improve performance and are resized/released as needed.")
            .size(10)
            .into()
    );

    // Show label
    items.push(
        row(vec![
            text("Show label").into(),
            toggler(config.memory.label_visible())
                .on_toggle(AppMessage::ToggleMemoryShowLabel)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show icon
    items.push(
        row(vec![
            text("Show icon").into(),
            toggler(config.memory.icon_visible())
                .on_toggle(AppMessage::ToggleMemoryShowIcon)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // As percentage
    items.push(
        row(vec![
            text("As percentage").into(),
            toggler(config.memory.percentage)
                .on_toggle(AppMessage::ToggleMemoryAsPercentage)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Colors button
    items.push(
        container(
            button::text("Colors")
        )
        .width(Length::Fill)
        .into()
    );

    let content = column(items)
        .spacing(16)
        .padding(24);
    
    let scrollable_content = scrollable(content)
        .height(Length::Shrink);
    
    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}

/// View for Network sensor configuration
pub fn view_network_config(config: &MinimonConfig) -> Element<'static, AppMessage> {
    let mut items: Vec<Element<AppMessage>> = vec![
        button::text("← Back")
            .on_press(AppMessage::Back)
            .into(),
        text("Network load").size(24).into(),
    ];

    // Combine download and upload
    items.push(
        row(vec![
            text("Combine download and upload").into(),
            toggler(config.network1.variant == NetworkVariant::Combined)
                .on_toggle(AppMessage::ToggleNetworkCombine)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show label
    items.push(
        row(vec![
            text("Show label").into(),
            toggler(config.network1.label_visible())
                .on_toggle(AppMessage::ToggleNetworkShowLabel)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show icon
    items.push(
        row(vec![
            text("Show icon").into(),
            toggler(config.network1.icon_visible())
                .on_toggle(AppMessage::ToggleNetworkShowIcon)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Section divider and header
    items.push(divider::horizontal::default().into());
    items.push(text("Network load in bytes per second").size(14).into());

    // Show sensor
    items.push(
        row(vec![
            text("Show sensor").into(),
            toggler(config.network1.chart_visible())
                .on_toggle(AppMessage::ToggleNetworkShowChart)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Colors button
    items.push(
        container(
            button::text("Colors")
        )
        .width(Length::Fill)
        .into()
    );

    let content = column(items)
        .spacing(16)
        .padding(24);
    
    let scrollable_content = scrollable(content)
        .height(Length::Shrink);
    
    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}

/// View for Disk sensor configuration
pub fn view_disk_config(config: &MinimonConfig) -> Element<'static, AppMessage> {
    let mut items: Vec<Element<AppMessage>> = vec![
        button::text("← Back")
            .on_press(AppMessage::Back)
            .into(),
        text("Disk load").size(24).into(),
    ];

    // Combine disk Write and Read
    items.push(
        row(vec![
            text("Combine disk Write and Read").into(),
            toggler(config.disks1.variant == DisksVariant::Combined)
                .on_toggle(AppMessage::ToggleDiskCombine)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show label
    items.push(
        row(vec![
            text("Show label").into(),
            toggler(config.disks1.label_visible())
                .on_toggle(AppMessage::ToggleDiskShowLabel)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show icon
    items.push(
        row(vec![
            text("Show icon").into(),
            toggler(config.disks1.icon_visible())
                .on_toggle(AppMessage::ToggleDiskShowIcon)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Disk write section
    items.push(divider::horizontal::default().into());
    items.push(text("Disk write in bytes per second").size(14).into());

    items.push(
        row(vec![
            text("Show sensor").into(),
            toggler(config.disks1.chart_visible())
                .on_toggle(AppMessage::ToggleDiskWriteShowChart)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    items.push(
        container(
            button::text("Colors")
        )
        .width(Length::Fill)
        .into()
    );

    // Disk read section
    items.push(divider::horizontal::default().into());
    items.push(text("Disk read in bytes per second").size(14).into());

    items.push(
        row(vec![
            text("Show sensor").into(),
            toggler(config.disks1.chart_visible())
                .on_toggle(AppMessage::ToggleDiskReadShowChart)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    items.push(
        container(
            button::text("Colors")
        )
        .width(Length::Fill)
        .into()
    );

    let content = column(items)
        .spacing(16)
        .padding(24);
    
    let scrollable_content = scrollable(content)
        .height(Length::Shrink);
    
    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}

/// View for GPU sensor configuration
pub fn view_gpu_config(config: &MinimonConfig) -> Element<'static, AppMessage> {
    let mut items: Vec<Element<AppMessage>> = vec![
        button::text("← Back")
            .on_press(AppMessage::Back)
            .into(),
        text("Graphics").size(24).into(),
        text("NVIDIA GeForce RTX 4090").size(16).into(),
    ];

    // Get default GPU config from HashMap
    let default_gpu = config.gpus.get("default");

    // Show label
    items.push(
        row(vec![
            text("Show label").into(),
            toggler(default_gpu.map(|g| g.usage.label_visible()).unwrap_or(false))
                .on_toggle(AppMessage::ToggleGpuShowLabel)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // Show icon
    items.push(
        row(vec![
            text("Show icon").into(),
            toggler(default_gpu.map(|g| g.usage.icon_visible()).unwrap_or(true))
                .on_toggle(AppMessage::ToggleGpuShowIcon)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // GPU load section
    items.push(divider::horizontal::default().into());
    items.push(text("GPU load").size(14).into());

    items.push(
        row(vec![
            text("Show sensor").into(),
            toggler(default_gpu.map(|g| g.usage.chart_visible()).unwrap_or(true))
                .on_toggle(AppMessage::ToggleGpuLoadShowChart)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    items.push(
        container(
            button::text("Colors")
        )
        .width(Length::Fill)
        .into()
    );

    // GPU VRAM section
    items.push(divider::horizontal::default().into());
    items.push(text("GPU VRAM").size(14).into());

    items.push(
        row(vec![
            text("Show sensor").into(),
            toggler(default_gpu.map(|g| g.vram.chart_visible()).unwrap_or(true))
                .on_toggle(AppMessage::ToggleGpuVramShowChart)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    // As percentage toggle
    items.push(
        row(vec![
            text("As percentage").into(),
            toggler(default_gpu.map(|g| g.vram.percentage).unwrap_or(false))
                .on_toggle(AppMessage::ToggleGpuVramAsPercentage)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    items.push(
        container(
            button::text("Colors")
        )
        .width(Length::Fill)
        .into()
    );

    // GPU Temperature section
    items.push(divider::horizontal::default().into());
    items.push(text("GPU Temperature").size(14).into());

    items.push(
        row(vec![
            text("Show sensor").into(),
            toggler(default_gpu.map(|g| g.temp.chart_visible()).unwrap_or(false))
                .on_toggle(AppMessage::ToggleGpuTempShowChart)
                .width(Length::Shrink)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    items.push(
        row(vec![
            text("Temperature unit").into(),
            button::text("Celsius ▾")
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    items.push(
        row(vec![
            text("Minimum temperature").into(),
            button::text("0")
                .width(Length::Fixed(80.0))
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    );

    items.push(
        container(
            button::text("Colors")
        )
        .width(Length::Fill)
        .into()
    );

    let content = column(items)
        .spacing(16)
        .padding(24);
    
    let scrollable_content = scrollable(content)
        .height(Length::Shrink);
    
    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}
