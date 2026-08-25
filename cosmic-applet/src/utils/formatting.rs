//! Formatting utilities for network rates and uptime.

use std::fmt;

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