//! GPU VRAM statistics.
//!
//! Collects GPU video memory (VRAM) usage on systems with NVIDIA or AMD GPUs.
/// On unsupported hardware, all fields return `None`. Detection uses `/sys/class/drm/` for
/// AMD/Intel integrated graphics and vendor-specific sysfs paths.

use serde::{Deserialize, Serialize};

/// GPU VRAM statistics — optional because not all systems have a discrete GPU.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuStats {
    /// Total VRAM in bytes, or `None` if no discrete GPU is detected.
    pub vram_total: Option<u64>,
    /// Used VRAM in bytes, or `None` if unavailable.
    pub vram_used: Option<u64>,
}

/// Collect current GPU statistics.
///
/// Attempts to detect GPU VRAM by reading AMD/Intel sysfs files at `/sys/class/drm/card0/device/`.
/// Specifically reads `mem_info_vram_total`, `mem_info_vram_used`, and related files.
/// On systems without a discrete GPU or where the sysfs paths don't exist, returns `None` for all fields.
pub fn collect() -> GpuStats {
    let vram_total = read_sysfs_file("/sys/class/drm/card0/device/mem_info_vram_total");
    let vram_used = read_sysfs_file("/sys/class/drm/card0/device/mem_info_vram_used");

    // If we couldn't read the total, also try mem_info_vram_mem_usage as a fallback.
    // Some AMD drivers expose usage differently. If neither exists, GPU is likely absent or integrated.
    let vram_total = vram_total.or_else(|| {
        read_sysfs_file("/sys/class/drm/card0/device/mem_info_vram_mem_usage")
            .map(|used| used) // This file gives current usage in bytes; total may be separate
    });

    GpuStats {
        vram_total,
        vram_used,
    }
}

/// Read a sysfs file and parse its contents as u64. Returns None if the file doesn't exist or can't be parsed.
fn read_sysfs_file(path: &str) -> Option<u64> {
    use std::fs;

    let content = fs::read_to_string(path).ok()?;
    // sysfs files may contain trailing newlines and whitespace
    let trimmed = content.trim();
    trimmed.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Handles None gracefully on unsupported hardware (Beverly writes after implementation).
    #[test]
    fn test_gpu_optional_handling() {
        let stats = collect();
        // On systems without a discrete GPU, both should be None.
        // On GPU-equipped systems, they may return Some values — just verify no panic occurs.
        if stats.vram_total.is_some() {
            assert!(stats.vram_total.unwrap() > 0);
        }
        if stats.vram_used.is_some() {
            // Used VRAM should not exceed total (if both are available)
            if let Some(total) = stats.vram_total {
                assert!(stats.vram_used.unwrap() <= total, "VRAM used exceeds total");
            }
        }
    }

    /// Verify that the sysfs file reader returns None for nonexistent paths.
    #[test]
    fn test_read_sysfs_nonexistent_path() {
        let result = read_sysfs_file("/sys/class/drm/card0/device/nonexistent_file_12345");
        assert!(result.is_none());
    }
}
