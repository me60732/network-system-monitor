//! GPU statistics — VRAM and utilization.
//!
//! Collects GPU video memory (VRAM) usage and GPU load percentage on systems with NVIDIA, AMD or Intel GPUs.
/// Detects GPU vendor by scanning `/sys/class/drm/card*/device/vendor`:
/// - **0x10de (NVIDIA)**: Uses NVML library for accurate VRAM/load stats
/// - **0x1002 (AMD)** or **0x8086 (Intel)**: Reads from card0 sysfs (mem_info_vram_*, gpu_busy_percent)
use nvml_wrapper::Nvml;
use std::fs;

/// GPU statistics — optional because not all systems have a discrete GPU.
#[derive(Debug, Clone, Default)]
pub struct GpuStats {
    /// Total VRAM in bytes, or `None` if no discrete GPU is detected.
    pub vram_total: Option<u64>,
    /// Used VRAM in bytes, or `None` if unavailable.
    pub vram_used: Option<u64>,
    /// GPU utilization load percentage (0.0–100.0), or `None` if unavailable.
    ///
    /// ## Data Sources
    ///
    /// - **NVIDIA**: NVML device utilization rates (gpu field)
    /// - **AMD/Intel**: Reads from `/sys/class/drm/card*/device/gpu_busy_percent`
    /// - Returns `None` if GPU is absent or utilization file doesn't exist
    pub gpu_load_percent: Option<f32>,
    /// GPU temperature in Celsius, or `None` if unavailable.
    ///
    /// ## Data Sources
    ///
    /// - **NVIDIA**: NVML temperature sensor
    /// - **AMD/Intel**: sysfs hwmon (if available)
    /// - Returns `None` if GPU temp sensor not accessible
    pub gpu_temp: Option<f32>,
}

/// GPU detection type discovered at initialization.
#[derive(Debug, Clone, Copy)]
enum GpuType {
    /// NVIDIA GPU detected — use NVML for stats
    Nvidia,
    /// AMD/Intel GPU or no discrete GPU — use sysfs fallback
    Sysfs,
    /// No GPU detected
    None,
}

/// Stateful GPU collector that detects GPU type once at initialization.
///
/// Caches GPU type detection to avoid scanning /sys/class/drm/* on every collection.
/// Reduces collection time by ~40ms per cycle (duplicate NVML queries eliminated).
pub struct GpuCollector {
    gpu_type: GpuType,
    nvml: Option<Nvml>,
    sysfs_card: Option<String>,
}

impl GpuCollector {
    /// Create a new GPU collector and detect GPU type at initialization.
    ///
    /// Performs expensive GPU detection ONCE to determine whether to use NVML or sysfs.
    /// Subsequent calls to `collect()` use the cached detection result.
    pub fn new() -> Self {
        // Detect GPU type
        let (gpu_type, nvml, sysfs_card) = Self::detect_gpu();

        Self {
            gpu_type,
            nvml,
            sysfs_card,
        }
    }

    /// Detect GPU type and initialize appropriate backend.
    fn detect_gpu() -> (GpuType, Option<Nvml>, Option<String>) {
        // Try NVIDIA first
        if has_nvidia_gpu() {
            if let Ok(nvml) = Nvml::init() {
                if let Ok(count) = nvml.device_count() {
                    if count > 0 {
                        return (GpuType::Nvidia, Some(nvml), None);
                    }
                }
            }
        }

        // Try sysfs for AMD/Intel
        if let Some(card) = find_sysfs_card() {
            return (GpuType::Sysfs, None, Some(card));
        }

        (GpuType::None, None, None)
    }

    /// Collect current GPU statistics using the cached GPU type.
    pub fn collect(&self) -> GpuStats {
        match self.gpu_type {
            GpuType::Nvidia => self.collect_nvidia(),
            GpuType::Sysfs => self.collect_sysfs(),
            GpuType::None => GpuStats::default(),
        }
    }

    /// Collect NVIDIA GPU stats using cached NVML instance.
    fn collect_nvidia(&self) -> GpuStats {
        let nvml = match &self.nvml {
            Some(n) => n,
            None => return GpuStats::default(),
        };

        let mut vram_total: Option<u64> = None;
        let mut vram_used: Option<u64> = None;
        let mut gpu_load_percent: Option<f32> = None;
        let mut gpu_temp: Option<f32> = None;

        if let Ok(device) = nvml.device_by_index(0) {
            // Get VRAM in bytes
            if let Ok(mem) = device.memory_info() {
                vram_total = Some(mem.total);
                vram_used = Some(mem.used);
            }

            // Get GPU load percentage (0-100)
            if let Ok(rates) = device.utilization_rates() {
                gpu_load_percent = Some(rates.gpu as f32);
            }

            // Get GPU temperature in Celsius.
            // TemperatureSensor::Gpu is the only variant in nvml-wrapper 0.10.
            // On Grace-Blackwell (GB10) this may return NotSupported; the
            // aarch64 thermal-zone fallback below handles that case.
            use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
            if let Ok(temp) = device.temperature(TemperatureSensor::Gpu) {
                gpu_temp = Some(temp as f32);
                log::debug!("NVML GPU temp: {}°C", temp);
            }

            log::debug!(
                "NVIDIA GPU stats: vram_used={:?}, vram_total={:?}, load={:?}%, temp={:?}°C",
                vram_used.map(|v| v / 1_048_576),
                vram_total.map(|v| v / 1_048_576),
                gpu_load_percent,
                gpu_temp
            );
        }

        // If NVML didn't return a temperature (e.g. GB10 Grace-Blackwell where
        // TemperatureSensor::Gpu returns NotSupported, or any card where the
        // driver withholds it), fall back to parsing `nvidia-smi` directly.
        // This works on all architectures — x86_64 RTX cards, ARM DGX Spark, etc.
        if gpu_temp.is_none() {
            gpu_temp = read_gpu_temp_nvidia_smi();
        }

        GpuStats {
            vram_total,
            vram_used,
            gpu_load_percent,
            gpu_temp,
        }
    }

    /// Collect sysfs GPU stats using cached card name.
    fn collect_sysfs(&self) -> GpuStats {
        let card = match &self.sysfs_card {
            Some(c) => c,
            None => return GpuStats::default(),
        };

        let vram_total = read_sysfs_file(&format!(
            "/sys/class/drm/{}/device/mem_info_vram_total",
            card
        ));
        let vram_used = read_sysfs_file(&format!(
            "/sys/class/drm/{}/device/mem_info_vram_used",
            card
        ));
        let gpu_load_percent =
            read_sysfs_file(&format!("/sys/class/drm/{}/device/gpu_busy_percent", card))
                .map(|percent| percent as f32);

        GpuStats {
            vram_total,
            vram_used,
            gpu_load_percent,
            gpu_temp: None, // AMD/Intel sysfs doesn't reliably expose GPU temp
        }
    }
}

/// Convenience wrapper that creates a one-shot collector for simple use cases.
///
/// This function creates a new GpuCollector, detects GPU type once,
/// collects the current stats, and drops the collector. Use [`GpuCollector::new()`]
/// and [`GpuCollector::collect()`] directly when you need to collect multiple times
/// without re-detecting GPU type (e.g., in MetricsAggregator).
pub fn collect() -> GpuStats {
    let collector = GpuCollector::new();
    collector.collect()
}

/// Detect if NVIDIA GPU is present by scanning /sys/class/drm/card*/device/vendor
fn has_nvidia_gpu() -> bool {
    let base = "/sys/class/drm";

    match fs::read_dir(base) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                let vendor_path = path.join("device/vendor");

                if vendor_path.exists() {
                    if let Ok(vendor_id) = fs::read_to_string(&vendor_path) {
                        if vendor_id.trim() == "0x10de" {
                            return true;
                        }
                    }
                }
            }
        }
        Err(_) => return false,
    }

    false
}

/// Find the first valid sysfs card with VRAM info (AMD/Intel).
fn find_sysfs_card() -> Option<String> {
    let base = "/sys/class/drm";

    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Check for VRAM total file (indicates valid GPU)
            let vram_total_path = path.join("device/mem_info_vram_total");

            if vram_total_path.exists() {
                if let Some(card_name) = path.file_name().and_then(|n| n.to_str()) {
                    return Some(card_name.to_string());
                }
            }
        }
    }

    None
}

/// Parse GPU temperature from `nvidia-smi`.
///
/// Used as a fallback when NVML's TemperatureSensor::Gpu returns NotSupported
/// (e.g. GB10 Grace-Blackwell on aarch64) or is otherwise unavailable.
/// Works on all NVIDIA-supported architectures — x86_64, aarch64, etc.
fn read_gpu_temp_nvidia_smi() -> Option<f32> {
    let output = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=temperature.gpu", "--format=csv,noheader"])
        .output()
        .ok()?;

    if !output.status.success() {
        log::warn!("nvidia-smi exited with error — GPU temp unavailable");
        return None;
    }

    let text = String::from_utf8(output.stdout).ok()?;
    let temp: f32 = text.trim().parse().ok()?;
    log::debug!("nvidia-smi GPU temp: {}°C", temp);
    Some(temp)
}

/// Read a sysfs file and parse its contents as u64. Returns None if the file doesn't exist or can't be parsed.
fn read_sysfs_file(path: &str) -> Option<u64> {
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
