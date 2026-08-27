//! GPU statistics — VRAM, utilization, and temperature.
//!
//! Vendor-specific modules for NVIDIA, AMD, and Intel GPUs.
//! Detects GPU vendor and dispatches to appropriate implementation.

use serde::{Deserialize, Serialize};

pub mod nvidia;
pub mod amd;
pub mod intel;

/// GPU statistics — optional because not all systems have a discrete GPU.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuStats {
    /// Total VRAM in bytes, or `None` if no discrete GPU is detected.
    pub vram_total: Option<u64>,
    /// Used VRAM in bytes, or `None` if unavailable.
    pub vram_used: Option<u64>,
    /// GPU utilization load percentage (0.0–100.0), or `None` if unavailable.
    pub gpu_load_percent: Option<f32>,
    /// GPU temperature in Celsius, or `None` if unavailable.
    pub gpu_temp: Option<f32>,
}

/// Collect current GPU statistics.
///
/// Tries vendors in order: NVIDIA (NVML) → AMD (sysfs) → Intel (sysfs).
/// Returns first successful collection or default (all None) if no GPU detected.
pub fn collect() -> GpuStats {
    // Try NVIDIA first (most reliable via NVML)
    if let Some(stats) = nvidia::collect() {
        return stats;
    }
    
    // Try AMD (sysfs)
    if let Some(stats) = amd::collect() {
        return stats;
    }
    
    // Try Intel (sysfs) - mostly unimplemented for now
    if let Some(stats) = intel::collect() {
        return stats;
    }
    
    // No GPU detected or all collection methods failed
    GpuStats::default()
}
