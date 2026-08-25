//! Color theming and threshold constants for metric visualization.

use cosmic::iced::{Color, Pixels};

/// Threshold constants for metric color coding (shared across all charts).
pub const THRESHOLD_WARN: f32 = 60.0;   // Yellow — approaching capacity
pub const THRESHOLD_CRIT: f32 = 80.0;   // Red — critical level

/// Color categories for metrics — determines ring/progress bar colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricColor {
    /// Green — healthy/normal (< 60%).
    Green,
    /// Yellow — warning range (60–80%). Approaching capacity, monitor closely.
    Yellow,
    /// Red — critical range (> 80%). Immediate attention required.
    Red,
    /// Gray — unavailable or not applicable (e.g., GPU VRAM on systems without discrete GPU).
    Gray,
}

impl MetricColor {
    /// Returns the CSS color string for this metric color category.
    pub fn as_color(&self) -> &'static str {
        match self {
            MetricColor::Green => "#4ade80",   // emerald-400 — healthy/normal.
            MetricColor::Yellow => "#facc15",  // yellow-400 — warning/approaching threshold.
            MetricColor::Red => "#f87171",     // red-400 — critical/exceeds threshold.
            MetricColor::Gray => "#9ca3af",    // gray-400 — unavailable/not applicable.
        }
    }

    /// Returns the cosmic::iced::Color for this metric color category.
    pub fn as_iced_color(&self) -> cosmic::iced::Color {
        match self {
            MetricColor::Green => Color::from_rgb8(0x4a, 0xde, 0x80),
            MetricColor::Yellow => Color::from_rgb8(0xfa, 0xcc, 0x15),
            MetricColor::Red => Color::from_rgb8(0xf8, 0x71, 0x71),
            MetricColor::Gray => Color::from_rgb8(0x9c, 0xa3, 0xaf),
        }
    }

    /// Returns the 60%/80% color category for a given percentage value (0.0–100.0).
    pub fn from_percentage(value: f32) -> Self {
        if value < THRESHOLD_WARN {   // < 60%: green
            MetricColor::Green
        } else if value < THRESHOLD_CRIT {  // 60–80%: yellow
            MetricColor::Yellow
        } else {
            MetricColor::Red
        }
    }
}

/// Status indicator for machine connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusIndicator {
    /// Whether the machine is currently online.
    pub online: bool,
    /// Seconds since last successful packet (for blinking logic).
    pub seconds_since_update: u64,
}

impl StatusIndicator {
    /// Create a new status indicator.
    pub fn new(online: bool) -> Self {
        StatusIndicator {
            online,
            seconds_since_update: 0,
        }
    }

    /// Returns the Unicode indicator symbol for this status: ● (online), ○ (offline/pending).
    pub fn symbol(&self) -> &'static str {
        if self.online {
            "●"   // Filled circle — machine is active.
        } else {
            "○"   // Hollow circle — inactive or pending.
        }
    }

    /// Returns the color for this status indicator (green=online, gray=offline/pending).
    pub fn color(&self) -> MetricColor {
        if self.online {
            MetricColor::Green
        } else {
            MetricColor::Gray
        }
    }
}

/// Format bytes per second into human-readable string (e.g., "1.2 MB/s").
pub fn format_network_rate(bytes_per_sec: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes_per_sec >= GB {
        format!("{:.1} GB/s", bytes_per_sec as f32 / GB as f32)
    } else if bytes_per_sec >= MB {
        format!("{:.1} MB/s", bytes_per_sec as f32 / MB as f32)
    } else if bytes_per_sec >= KB {
        format!("{:.1} KB/s", bytes_per_sec as f32 / KB as f32)
    } else {
        format!("{} B/s", bytes_per_sec)
    }
}

/// Format seconds into human-readable uptime (e.g., "3d 4h").
pub fn format_uptime(seconds: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = MINUTE * 60;
    const DAY: u64 = HOUR * 24;

    let days = seconds / DAY;
    let hours = (seconds % DAY) / HOUR;
    let minutes = (seconds % HOUR) / MINUTE;

    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m", minutes)
    } else {
        format!("{}s", seconds)
    }
}