//! # MainMenu — Sensor configuration menu
//!
//! Shows a dark-themed menu with:
//! - Header button: "COSMIC System Monitor" (opens external app)
//! - General settings option
//! - Individual sensor configuration options with live values

use crate::{AppMessage, config::manager::ConfigManager};
use cosmic::widget::{button, column, container, divider, row, scrollable, text, icon, mouse_area};
use cosmic::iced::{Length, Alignment};
use cosmic::Element;

/// View the main menu with sensor configuration options and live metrics
pub fn view(
    _config_manager: &std::sync::Arc<std::sync::RwLock<ConfigManager>>,
    machines: &[crate::remote_machine::RemoteMachine],
    _minimon_config: &crate::minimon_config::MinimonConfig,
) -> Element<'static, AppMessage> {
    // Aggregate metrics from all machines for display
    let (avg_cpu, avg_cpu_temp, memory_text, network_text, disk_text, gpu_text) = 
        aggregate_sensor_data(machines);
    
    // Back button to return to panel view
    let back_button = button::text("← Close Menu")
        .on_press(AppMessage::Back)
        .width(Length::Fill);
    
    // Header button - COSMIC System Monitor (external app)
    let header_button = button::custom(
        row(vec![
            text("COSMIC System Monitor").size(14).into(),
            icon::from_name("send-to-symbolic").size(14).into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
    )
    .on_press(AppMessage::NoOp)  // TODO: launch external app
    .width(Length::Fill);
    
    // General settings option
    let general_settings_button = create_menu_item(
        "General settings",
        "",
        AppMessage::OpenGeneralSettings
    );
    
    // Individual sensor configuration options with live values
    let cpu_button = create_menu_item(
        "CPU",
        &format!("{:.2}% / {}°C", avg_cpu, avg_cpu_temp as i32),
        AppMessage::OpenCpuConfig
    );
    
    let memory_button = create_menu_item(
        "Memory",
        &memory_text,
        AppMessage::OpenMemoryConfig
    );
    
    let network_button = create_menu_item(
        "Network",
        &network_text,
        AppMessage::OpenNetworkConfig
    );
    
    let disk_button = create_menu_item(
        "Disk",
        &disk_text,
        AppMessage::OpenDiskConfig
    );
    
    let gpu_button = create_menu_item(
        "GPU",
        &gpu_text,
        AppMessage::OpenGpuConfig
    );
    
    // Build content column
    let content = column(vec![
        container(back_button)
            .padding([8, 16])
            .width(Length::Fill)
            .into(),
        divider::horizontal::default().into(),
        container(header_button)
            .padding([8, 16])
            .width(Length::Fill)
            .into(),
        divider::horizontal::default().into(),
        general_settings_button,
        divider::horizontal::default().into(),
        cpu_button,
        memory_button,
        network_button,
        disk_button,
        gpu_button,
    ])
    .spacing(0);
    
    // Wrap in scrollable with max height to enable scrollbar when content is too long
    let scrollable_content = scrollable(content)
        .height(Length::Shrink);
    
    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}

/// Create a menu item row with label, value, and chevron
fn create_menu_item(
    label: &str,
    value: &str,
    message: AppMessage,
) -> Element<'static, AppMessage> {
    let label_text = label.to_string();
    let value_text = value.to_string();
    
    mouse_area(
        container(
            row(vec![
                text(label_text).width(Length::Fill).into(),
                text(value_text).size(14).into(),
                icon::from_name("go-next-symbolic").size(16).into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .padding([12, 16])
        )
        .width(Length::Fill)
    )
    .on_press(message)
    .into()
}

/// Aggregate sensor data from all machines
fn aggregate_sensor_data(machines: &[crate::remote_machine::RemoteMachine]) -> (f32, f32, String, String, String, String) {
    if machines.is_empty() {
        return (0.0, 0.0, "—".into(), "—".into(), "—".into(), "—".into());
    }
    
    let mut total_cpu = 0.0;
    let mut total_cpu_temp = 0.0;
    let mut total_mem_used = 0u64;
    let mut total_mem_available = 0u64;
    let mut total_mem_total = 0u64;
    let mut total_rx = 0.0;
    let mut total_tx = 0.0;
    let mut total_disk_write = 0.0;
    let mut total_disk_read = 0.0;
    let mut total_gpu_usage = 0.0;
    let mut total_gpu_mem_used = 0u64;
    let mut total_gpu_mem_total = 0u64;
    let mut total_gpu_temp = 0.0;
    
    for machine in machines {
        total_cpu += machine.sensors.cpu.usage_percent;
        total_cpu_temp += machine.sensors.temperature.celsius;
        total_mem_used += machine.sensors.memory.used_bytes;
        total_mem_available += machine.sensors.memory.total_bytes.saturating_sub(machine.sensors.memory.used_bytes);
        total_mem_total += machine.sensors.memory.total_bytes;
        total_rx += machine.sensors.network.rx_bytes_per_sec as f64;
        total_tx += machine.sensors.network.tx_bytes_per_sec as f64;
        total_disk_write += machine.sensors.disk.write_bytes_per_sec as f64;
        total_disk_read += machine.sensors.disk.read_bytes_per_sec as f64;
        total_gpu_usage += machine.sensors.gpu.usage_percent();
        total_gpu_mem_used += machine.sensors.gpu.vram_used_bytes;
        total_gpu_mem_total += machine.sensors.gpu.vram_total_bytes;
        total_gpu_temp += machine.sensors.temperature.celsius;  // Using CPU temp as proxy
    }
    
    let count = machines.len() as f32;
    let avg_cpu = total_cpu / count;
    let avg_cpu_temp = total_cpu_temp / count;
    let avg_gpu_usage = total_gpu_usage / count;
    let avg_gpu_temp = total_gpu_temp / count;
    
    // Format memory: used / available / total in GB
    let memory_text = format!(
        "{:.1} GB / {:.1} GB / {:.1} GB",
        total_mem_used as f64 / 1_073_741_824.0,
        total_mem_available as f64 / 1_073_741_824.0,
        total_mem_total as f64 / 1_073_741_824.0
    );
    
    // Format network: ↓ RX KB/s ↑ TX KB/s
    let network_text = format!(
        "↓ {:.2} KB/s ↑ {:.2} KB/s",
        total_rx / 1024.0,
        total_tx / 1024.0
    );
    
    // Format disk: w WRITE r READ
    let disk_text = if total_disk_write > 1_073_741_824.0 {
        format!(
            "w {:.2} GB/s r {:.2} MB/s",
            total_disk_write / 1_073_741_824.0,
            total_disk_read / 1_048_576.0
        )
    } else {
        format!(
            "w {:.2} MB/s r {:.2} MB/s",
            total_disk_write / 1_048_576.0,
            total_disk_read / 1_048_576.0
        )
    };
    
    // Format GPU: usage% VRAM used / VRAM total temp°C
    let gpu_text = format!(
        "{:.2}% {:.2} GB / {:.2} GB {}°C",
        avg_gpu_usage,
        total_gpu_mem_used as f64 / 1_073_741_824.0,
        total_gpu_mem_total as f64 / 1_073_741_824.0,
        avg_gpu_temp as i32
    );
    
    (avg_cpu, avg_cpu_temp, memory_text, network_text, disk_text, gpu_text)
}
