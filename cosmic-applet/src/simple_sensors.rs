//! Simplified sensors for network monitoring - data + rendering only, no settings UI.
//!
//! These sensors receive MetricPacket data from UDP and render themselves in the grid view.
//! Unlike minimon's full sensors, these have no configuration UI, no toggles, no settings.

use cosmic::Element;
use nmd_service::packet::MetricPacket;

/// CPU usage data
#[derive(Debug, Clone)]
pub struct CpuData {
    pub usage_percent: f32,
    pub temperature_celsius: Option<f32>,
    pub cores: Vec<f32>, // Per-core usage
}

impl Default for CpuData {
    fn default() -> Self {
        Self {
            usage_percent: 0.0,
            temperature_celsius: None,
            cores: Vec::new(),
        }
    }
}

impl CpuData {
    pub fn update_from_packet(&mut self, packet: &MetricPacket) {
        // Phase 3: Extract CPU metrics from nested group
        self.usage_percent = packet.cpu.usage_percent;
        self.temperature_celsius = packet.cpu.temperature_celsius;

        log::debug!(
            "📊 CPU packet data - usage: {:.1}%, temp: {:?}",
            self.usage_percent,
            self.temperature_celsius
        );

        // TODO: Extract per-core data when available
    }

    pub fn render(&self) -> Element<'static, crate::AppMessage> {
        use crate::charts::ring::RingChart;
        use crate::minimon_config::ChartColors;
        use cosmic::widget::canvas;

        let colors = ChartColors::new(
            crate::minimon_config::DeviceKind::Cpu,
            crate::minimon_config::ChartKind::Ring,
        );

        let ring = RingChart::new(self.usage_percent, &colors);

        canvas(ring).width(28).height(28).into()
    }
}

/// Memory usage data
#[derive(Debug, Clone)]
pub struct MemoryData {
    pub used_bytes: u64,
    pub total_bytes: u64,
}

impl Default for MemoryData {
    fn default() -> Self {
        Self {
            used_bytes: 0,
            total_bytes: 1, // Avoid division by zero
        }
    }
}

impl MemoryData {
    pub fn update_from_packet(&mut self, packet: &MetricPacket) {
        // Phase 3: Extract memory metrics from nested group
        self.used_bytes = packet.memory.used_bytes;
        self.total_bytes = packet.memory.total_bytes;

        log::debug!(
            "📊 Memory packet data - used: {} bytes, total: {} bytes",
            self.used_bytes,
            self.total_bytes
        );
    }

    pub fn usage_percent(&self) -> f32 {
        if self.total_bytes > 0 {
            (self.used_bytes as f64 / self.total_bytes as f64 * 100.0) as f32
        } else {
            0.0
        }
    }

    pub fn render(&self) -> Element<'static, crate::AppMessage> {
        use crate::charts::ring::RingChart;
        use crate::minimon_config::ChartColors;
        use cosmic::widget::canvas;

        let colors = ChartColors::new(
            crate::minimon_config::DeviceKind::Memory,
            crate::minimon_config::ChartKind::Ring,
        );

        let ring = RingChart::new(self.usage_percent(), &colors);

        canvas(ring).width(28).height(28).into()
    }
}

/// Network I/O data
#[derive(Debug, Clone)]
pub struct NetworkData {
    pub rx_bytes_per_sec: u64,
    pub tx_bytes_per_sec: u64,
}

impl Default for NetworkData {
    fn default() -> Self {
        Self {
            rx_bytes_per_sec: 0,
            tx_bytes_per_sec: 0,
        }
    }
}

impl NetworkData {
    pub fn update_from_packet(&mut self, packet: &MetricPacket) {
        // Packet contains bytes transferred since last collection interval (~1 second)
        // Use directly as bytes-per-second rate (no delta computation needed)
        self.rx_bytes_per_sec = packet.network.rx_bytes;
        self.tx_bytes_per_sec = packet.network.tx_bytes;
    }

    pub fn render(&self) -> Element<'static, crate::AppMessage> {
        use cosmic::widget::{column, text};

        // Convert bytes/sec to KB/s and format with adaptive unit scaling
        let rx_kbps = self.rx_bytes_per_sec as f64 / 1024.0;
        let tx_kbps = self.tx_bytes_per_sec as f64 / 1024.0;
        let rx_str = crate::utils::formatting::format_throughput_adaptive(rx_kbps);
        let tx_str = crate::utils::formatting::format_throughput_adaptive(tx_kbps);

        // Stack download and upload vertically
        column![
            text(format!("↓{}", rx_str)).size(9),
            text(format!("↑{}", tx_str)).size(9)
        ]
        .spacing(1)
        .into()
    }
}

/// Disk I/O data
#[derive(Debug, Clone)]
pub struct PartitionInfo {
    pub mount: String,
    pub total: u64,
    pub used: u64,
}

#[derive(Debug, Clone)]
pub struct DiskData {
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
    pub partitions: Vec<PartitionInfo>,
}

impl Default for DiskData {
    fn default() -> Self {
        Self {
            read_bytes_per_sec: 0,
            write_bytes_per_sec: 0,
            partitions: Vec::new(),
        }
    }
}

impl DiskData {
    pub fn update_from_packet(&mut self, packet: &MetricPacket) {
        // Packet contains bytes transferred since last collection interval (~1 second)
        // Use directly as bytes-per-second rate (no delta computation needed)
        self.read_bytes_per_sec = packet.disk.read_bytes.unwrap_or(0);
        self.write_bytes_per_sec = packet.disk.write_bytes.unwrap_or(0);

        // Update partitions from packet (nested field)
        self.partitions = packet
            .disk
            .partitions
            .iter()
            .map(|p| PartitionInfo {
                mount: p.mount.clone(),
                total: p.total,
                used: p.used,
            })
            .collect();
    }

    pub fn render(&self) -> Element<'static, crate::AppMessage> {
        use cosmic::widget::text;

        // Convert bytes/sec to KB/s and format with adaptive unit scaling
        let total_kbps = (self.read_bytes_per_sec + self.write_bytes_per_sec) as f64 / 1024.0;
        let formatted = crate::utils::formatting::format_throughput_adaptive(total_kbps);

        // Disk is text-only, no chart
        text(format!("DISK {}", formatted)).size(10).into()
    }
}

/// GPU VRAM usage data (includes utilization load and temperature)
#[derive(Debug, Clone)]
pub struct GpuData {
    pub vram_used_bytes: u64,
    pub vram_total_bytes: u64,
    /// GPU utilization load percentage (0.0–100.0), or `None` if unavailable
    pub load_percent: Option<f32>,
    /// GPU junction temperature in Celsius, or `None` if unavailable
    pub gpu_temp: Option<f32>,
}

impl Default for GpuData {
    fn default() -> Self {
        Self {
            vram_used_bytes: 0,
            vram_total_bytes: 1, // Avoid division by zero
            load_percent: None,  // Phase 2.1: GPU utilization (optional)
            gpu_temp: None,      // GPU temperature (optional)
        }
    }
}

impl GpuData {
    pub fn update_from_packet(&mut self, packet: &MetricPacket) {
        // Phase 3: Extract GPU metrics from nested group
        // Convert VRAM from MB to bytes for applet display
        if let Some(vram_mb) = packet.gpu.vram_used_mb {
            self.vram_used_bytes = vram_mb as u64 * 1_048_576; // Convert MB to bytes
            log::debug!(
                "📊 GPU VRAM used: {} MB → {} bytes",
                vram_mb,
                self.vram_used_bytes
            );
        }

        if let Some(vram_total_mb) = packet.gpu.vram_total_mb {
            self.vram_total_bytes = vram_total_mb as u64 * 1_048_576; // Convert MB to bytes
            log::debug!(
                "📊 GPU VRAM total: {} MB → {} bytes",
                vram_total_mb,
                self.vram_total_bytes
            );
        }

        // Phase 2.1: Extract GPU utilization load percentage (optional)
        self.load_percent = packet.gpu.load_percent;
        log::debug!("📊 GPU load percent: {:?}", self.load_percent);

        // Extract GPU temperature from nested field (separate from CPU temp)
        self.gpu_temp = packet.gpu.temperature_celsius;
        log::debug!("📊 GPU temperature: {:?}", self.gpu_temp);
    }

    pub fn usage_percent(&self) -> f32 {
        if self.vram_total_bytes > 0 {
            (self.vram_used_bytes as f64 / self.vram_total_bytes as f64 * 100.0) as f32
        } else {
            0.0
        }
    }

    pub fn render(&self) -> Element<'static, crate::AppMessage> {
        use crate::charts::ring::RingChart;
        use crate::minimon_config::{ChartColors, DeviceKind};
        use cosmic::widget::{canvas, row, text};

        // If GPU load is available, show it as the primary metric
        if let Some(load) = self.load_percent {
            let colors = ChartColors::new(DeviceKind::Gpu, crate::minimon_config::ChartKind::Ring);

            let ring = RingChart::new(load as f32, &colors);

            row![
                canvas(ring).width(28).height(28),
                text(format!("GPU {:.0}%", load)).size(10)
            ]
            .spacing(4)
            .into()
        } else {
            // Fallback: show VRAM usage if GPU load unavailable
            let colors = ChartColors::new(DeviceKind::Vram, crate::minimon_config::ChartKind::Ring);

            let ring = RingChart::new(self.usage_percent(), &colors);

            let gb_used = self.vram_used_bytes as f64 / 1_073_741_824.0;

            row![
                canvas(ring).width(28).height(28),
                text(format!("VRAM {:.1}GB", gb_used)).size(10)
            ]
            .spacing(4)
            .into()
        }
    }
}

/// Temperature data
#[derive(Debug, Clone)]
pub struct TemperatureData {
    pub celsius: f32,
}

impl Default for TemperatureData {
    fn default() -> Self {
        Self { celsius: 0.0 }
    }
}

impl TemperatureData {
    pub fn update_from_packet(&mut self, packet: &MetricPacket) {
        // Phase 3: Extract CPU temperature from nested group
        if let Some(temp) = packet.cpu.temperature_celsius {
            self.celsius = temp;
            log::debug!("📊 Temperature packet data - celsius: {}", self.celsius);
        }
    }

    pub fn render(&self) -> Element<'static, crate::AppMessage> {
        use crate::charts::ring::RingChart;
        use crate::minimon_config::{ChartColors, DeviceKind};
        use cosmic::widget::{canvas, row, text};

        let colors = ChartColors::new(DeviceKind::GpuTemp, crate::minimon_config::ChartKind::Ring);

        // Scale temperature to percentage (0°C = 0%, 100°C = 100%)
        let percent = (self.celsius / 100.0 * 100.0).min(100.0).max(0.0);

        let ring = RingChart::new_with_text(percent, &format!("{:.0}", self.celsius), &colors);

        row![
            canvas(ring).width(28).height(28),
            text(format!("TEMP {:.0}°C", self.celsius)).size(10)
        ]
        .spacing(4)
        .into()
    }
}

/// Combined sensor data for one remote machine
#[derive(Debug, Clone, Default)]
pub struct RemoteSensors {
    pub cpu: CpuData,
    pub memory: MemoryData,
    pub network: NetworkData,
    pub disk: DiskData,
    pub gpu: GpuData,
    pub temperature: TemperatureData,
    pub uptime_seconds: u64,
}

impl RemoteSensors {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update all sensors from an incoming UDP packet
    pub fn update_from_packet(&mut self, packet: &MetricPacket) {
        self.cpu.update_from_packet(packet);
        self.memory.update_from_packet(packet);
        self.network.update_from_packet(packet);
        self.disk.update_from_packet(packet);
        self.gpu.update_from_packet(packet);
        self.temperature.update_from_packet(packet);

        self.uptime_seconds = packet.uptime_seconds;
    }

    /// Render all sensors as a horizontal row of ring charts (minimon style)
    pub fn render(&self) -> Element<'static, crate::AppMessage> {
        use cosmic::widget::row;

        row![
            self.cpu.render(),
            self.memory.render(),
            self.network.render(),
            self.disk.render(),
            self.gpu.render(),
            self.temperature.render(),
        ]
        .spacing(4)
        .into()
    }
}
