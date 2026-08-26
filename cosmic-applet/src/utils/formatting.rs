//! Formatting utilities for network rates and uptime.

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

/// Format throughput with adaptive unit scaling to keep digit count small.
/// Switches units at >= 100 threshold to maintain 2-3 digit display.
/// Used for both network and disk I/O rates.
pub fn format_throughput_adaptive(kbps: f64) -> String {
    const THRESHOLD: f64 = 100.0;
    
    if kbps >= THRESHOLD * 1024.0 {
        // GB/s range (>= 100 MB/s = 102400 KB/s)
        format!("{:.1} GB/s", kbps / 1024.0 / 1024.0)
    } else if kbps >= THRESHOLD {
        // MB/s range (>= 100 KB/s)
        format!("{:.1} MB/s", kbps / 1024.0)
    } else {
        // KB/s range (< 100 KB/s)
        format!("{:.1} KB/s", kbps)
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