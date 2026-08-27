//! NVIDIA GPU statistics via NVML library.

use nvml_wrapper::Nvml;
use std::sync::OnceLock;
use log::debug;

use super::GpuStats;

/// Collect NVIDIA GPU stats using NVML.
///
/// Returns Some(GpuStats) if NVIDIA GPU detected and NVML available, None otherwise.
pub fn collect() -> Option<GpuStats> {
    static NVML: OnceLock<Option<Nvml>> = OnceLock::new();
    
    let nvml = match NVML.get_or_init(|| {
        Nvml::init().ok()
    }) {
        Some(n) => n,
        None => return None,
    };
    
    // Try to get device count
    let device_count = match nvml.device_count() {
        Ok(count) => count,
        Err(_) => return None,
    };
    
    if device_count == 0 {
        return None;
    }
    
    // Use first GPU (index 0)
    let device = match nvml.device_by_index(0) {
        Ok(d) => d,
        Err(_) => return None,
    };
    
    // Get VRAM in bytes
    let (vram_total, vram_used) = match device.memory_info() {
        Ok(mem) => (Some(mem.total), Some(mem.used)),
        Err(_) => (None, None),
    };
    
    // Get GPU load percentage (0-100)
    let gpu_load_percent = match device.utilization_rates() {
        Ok(rates) => Some(rates.gpu as f32),
        Err(_) => None,
    };
    
    // Get GPU temperature in Celsius
    let gpu_temp = match device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu) {
        Ok(temp) => Some(temp as f32),
        Err(_) => None,
    };
    
    debug!("NVIDIA GPU stats: vram_used={:?}, vram_total={:?}, load={:?}%, temp={:?}°C", 
           vram_used.map(|v| v / 1_048_576), vram_total.map(|v| v / 1_048_576), gpu_load_percent, gpu_temp);
    
    Some(GpuStats {
        vram_total,
        vram_used,
        gpu_load_percent,
        gpu_temp,
    })
}
