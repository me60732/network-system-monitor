//! # MachineSensorConfigMenu — Per-machine sensor row configuration menu
//!
//! Shows sensor configuration options for a specific machine with its live values.
//! Accessed from machine_detail via the settings button.

use crate::AppMessage;
use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{button, column, container, divider, icon, mouse_area, row, scrollable, text};

/// View the machine sensor config menu for a specific machine
pub fn view(
    machine_name: &str,
    machine: Option<&crate::remote_machine::RemoteMachine>,
    _sensor_config: &crate::minimon_config::MachineSensorConfig,
) -> Element<'static, AppMessage> {
    let _machine_name = machine_name.to_string();

    // Aggregate sensor live values from this machine only
    let (cpu_text, memory_text, network_text, disk_text, gpu_text) = if let Some(m) = machine {
        get_machine_sensor_data(m)
    } else {
        ("—".into(), "—".into(), "—".into(), "—".into(), "—".into())
    };

    let back_button = button::text("← Back")
        .on_press(AppMessage::Back)
        .width(Length::Fill);

    let cpu_button = create_menu_item("CPU", &cpu_text, AppMessage::OpenCpuConfig);
    let memory_button = create_menu_item("Memory", &memory_text, AppMessage::OpenMemoryConfig);
    let network_button = create_menu_item("Network", &network_text, AppMessage::OpenNetworkConfig);
    let disk_button = create_menu_item("Disk", &disk_text, AppMessage::OpenDiskConfig);
    let gpu_button = create_menu_item("GPU", &gpu_text, AppMessage::OpenGpuConfig);

    let content = column(vec![
        container(back_button)
            .padding([8, 16])
            .width(Length::Fill)
            .into(),
        divider::horizontal::default().into(),
        cpu_button,
        memory_button,
        network_button,
        disk_button,
        gpu_button,
    ])
    .spacing(0);

    let scrollable_content = scrollable(content).height(Length::Shrink);

    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}

fn create_menu_item(label: &str, value: &str, message: AppMessage) -> Element<'static, AppMessage> {
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
            .padding([12, 16]),
        )
        .width(Length::Fill),
    )
    .on_press(message)
    .into()
}

fn get_machine_sensor_data(
    m: &crate::remote_machine::RemoteMachine,
) -> (String, String, String, String, String) {
    let cpu_text = format!(
        "{:.1}% / {}°C",
        m.sensors.cpu.usage_percent, m.sensors.temperature.celsius as i32
    );

    let mem_used_gb = m.sensors.memory.used_bytes as f64 / 1_073_741_824.0;
    let mem_total_gb = m.sensors.memory.total_bytes as f64 / 1_073_741_824.0;
    let memory_text = format!("{:.1} / {:.1} GB", mem_used_gb, mem_total_gb);

    let rx_kbps = m.sensors.network.rx_bytes_per_sec as f64 / 1024.0;
    let tx_kbps = m.sensors.network.tx_bytes_per_sec as f64 / 1024.0;
    let network_text = format!("↓ {:.1} ↑ {:.1} KB/s", rx_kbps, tx_kbps);

    let disk_write = m.sensors.disk.write_bytes_per_sec as f64 / 1_048_576.0;
    let disk_read = m.sensors.disk.read_bytes_per_sec as f64 / 1_048_576.0;
    let disk_text = format!("W {:.1} R {:.1} MB/s", disk_write, disk_read);

    let gpu_usage = m.sensors.gpu.load_percent.unwrap_or(0.0);
    let gpu_vram_gb = m.sensors.gpu.vram_used_bytes as f64 / 1_073_741_824.0;
    let gpu_text = format!("{:.1}% {:.1} GB VRAM", gpu_usage, gpu_vram_gb);

    (cpu_text, memory_text, network_text, disk_text, gpu_text)
}
