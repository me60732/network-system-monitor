//! # MachineCard — Single row in GridWindow representing one remote machine
//!
//! Each row displays a machine's name, status (Online/Offline/Pending), and current metrics.
//! Metrics are rendered with color-coded progress bars at 60%/80% thresholds. Status indicators
//! show ● (online) or ○ (offline/pending).

use crate::charts::theme::{StatusIndicator, format_network_rate, format_uptime};

/// Status of a remote machine — determines the indicator symbol and row styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineStatus {
    /// Machine has sent at least one valid UDP packet recently (within timeout).
    Online,
    /// No packets received within expected interval — connection may be lost.
    Offline,
    /// First appearance in config but no data received yet.
    Pending,
}

impl MachineStatus {
    /// Returns the Unicode indicator symbol for this status: ● (online), ○ (offline/pending).
    pub fn symbol(&self) -> &'static str {
        match self {
            MachineStatus::Online => "●",   // Filled circle — machine is active.
            MachineStatus::Offline | MachineStatus::Pending => "○",  // Hollow circle — inactive or pending.
        }
    }

    /// Returns the color for this status indicator (green=online, gray=offline/pending).
    pub fn color(&self) -> crate::charts::theme::MetricColor {
        match self {
            MachineStatus::Online => crate::charts::theme::MetricColor::Green,
            MachineStatus::Offline | MachineStatus::Pending => crate::charts::theme::MetricColor::Gray,
        }
    }
}

/// A single row in GridWindow representing one remote machine's current state.
///
/// Contains the machine name, connection status, and all metric values received via UDP packets.
/// Metrics are updated by UdpReceiver when new MetricPacket data arrives for this machine_id.
#[derive(Clone)]
pub struct MachineCard {
    /// Machine identifier (hostname or configured name) — matches nmd-service's machine_id.
    pub name: String,

    /// Current connection status — Online/Offline/Pending determines indicator symbol and color.
    pub status: MachineStatus,

    // ── Metric Fields (updated by UDP receiver from rkyv-encoded MetricPacket) ───────────

    /// CPU usage percentage (0.0–100.0).
    pub cpu_usage: Option<f32>,

    /// Memory usage percentage (0.0–100.0).
    pub memory_usage: Option<f32>,

    /// Disk usage percentage (0.0–100.0).
    pub disk_usage: Option<f32>,

    /// Network RX rate in bytes/second.
    pub network_rx_bytes: u64,

    /// Network TX rate in bytes/second.
    pub network_tx_bytes: u64,

    /// System uptime in seconds.
    pub uptime_seconds: u64,

    /// GPU VRAM usage percentage (0.0–100.0), if available.
    pub gpu_vram_usage: Option<f32>,

    /// Current temperature in degrees Celsius.
    pub temperature_celsius: Option<f32>,
}

impl MachineCard {
    /// Create a new MachineCard with the given name and initial status.
    pub fn new(name: String, status: MachineStatus) -> Self {
        MachineCard {
            name,
            status,
            cpu_usage: None,
            memory_usage: None,
            disk_usage: None,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            uptime_seconds: 0,
            gpu_vram_usage: None,
            temperature_celsius: None,
        }
    }

    /// Update this row's metrics from an incoming MetricPacket.
    pub fn update_from_packet(&mut self, packet: &nmd_service::packet::MetricPacket) {
        // Update connection status based on packet receipt
        self.status = MachineStatus::Online;

        // Update metric values from the packet (wrap non-Option values in Some)
        self.cpu_usage = Some(packet.cpu_usage);
        self.memory_usage = Some(packet.memory_used_percent);
        self.disk_usage = Some(packet.disk_used_percent);
        self.network_rx_bytes = packet.network_rx_bytes;
        self.uptime_seconds = packet.uptime_seconds;
        
        // Optional fields
        self.gpu_vram_usage = packet.gpu_vram_used_mb.map(|v| (v as f32 / 8192.0) * 100.0);
        self.temperature_celsius = packet.temperature_celsius;
    }

    /// Update this row's metrics from an archived rkyv MetricPacket.
    pub fn update_from_archived(&mut self, packet: &nmd_service::packet::ArchivedMetricPacket) {
        // Update connection status based on packet receipt
        self.status = MachineStatus::Online;

        // Convert f32_le to f32 for direct fields (no ArchivedOption wrapping)
        self.cpu_usage = Some(packet.cpu_usage.into());
        self.memory_usage = Some(packet.memory_used_percent.into());
        self.disk_usage = Some(packet.disk_used_percent.into());
        
        // Convert fixed-length byte arrays and u64_le fields
        self.network_rx_bytes = packet.network_rx_bytes.into();
        self.uptime_seconds = packet.uptime_seconds.into();
        
        // Handle optional fields with proper conversion (ArchivedOption)
        self.gpu_vram_usage = match packet.gpu_vram_used_mb.as_ref() {
            Some(v) => {
                // Convert u32_le to f32 for percentage calculation (assuming max VRAM ~8GB)
                let vram_mb: u32 = (*v).into();
                Some((vram_mb as f32 / 8192.0) * 100.0)  // Assume 8GB max VRAM
            }
            None => None
        };
        
        self.temperature_celsius = match packet.temperature_celsius.as_ref() { Some(v) => Some((*v).into()), None => None };
    }

    /// Check if the machine is offline based on last update timestamp.
    /// Returns true if last_update is older than timeout_secs.
    pub fn is_offline(&self, _timeout_secs: u64) -> bool {
        // For now, return false since we don't track last_update timestamps
        // TODO: Add last_update field and implement proper offline detection (Beverly)
        false
    }
}

impl Default for MachineCard {
    fn default() -> Self {
        MachineCard::new("unknown".to_string(), MachineStatus::Pending)
    }
}
