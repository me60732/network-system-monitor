//! System temperature statistics.
//!
/// Collects CPU and GPU temperatures in Celsius using `/sys/class/hwmon/` on Linux.
/// Prioritizes Tdie/Tctl labels for AMD CPUs, and core/Package labels for Intel CPUs (like minimon-applet).
/// On unsupported hardware, fields return `None`.

use log::info;
use serde::{Deserialize, Serialize};

/// Temperature readings from system sensors — optional because not all systems expose thermal data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemperatureStats {
    /// CPU package temperature in Celsius, or `None` if unavailable.
    pub cpu_temp: Option<f32>,
    /// GPU junction/edge temperature in Celsius, or `None` if no discrete GPU detected.
    pub gpu_temp: Option<f32>,
}

/// Collect current temperature statistics.
///
/// Reads CPU temperature from `/sys/class/hwmon/` with sensor label prioritization:
/// - **AMD**: Tdie > Tctl > Core* > Package*
/// - **Intel**: Core* > Package*
/// For GPU temperature, prioritizes gpu::collect().gpu_temp (NVML for NVIDIA) over sysfs fallback.
/// Returns `None` for any sensor that cannot be read, rather than panicking — this allows the system
/// to gracefully degrade on hardware without thermal sensors (e.g., some virtual machines or containers).
pub fn collect() -> TemperatureStats {
    let cpu_temp = find_cpu_temperature();
    
    // For GPU temp, prioritize GPU collector (which uses NVML for NVIDIA)
    let gpu_stats = crate::gpu::collect();
    let gpu_temp = gpu_stats.gpu_temp.or_else(|| read_gpu_temperature());

    TemperatureStats {
        cpu_temp,
        gpu_temp,
    }
}

/// Find and read the most relevant CPU temperature sensor using hwmon label prioritization.
/// AMD: Tdie > Tctl > Core* > Package*
/// Intel: Core* > Package*
fn find_cpu_temperature() -> Option<f32> {
    use std::fs;
    use std::path::Path;

    let hwmon_base = Path::new("/sys/class/hwmon");

    for entry in fs::read_dir(hwmon_base).ok()? {
        let hwmon_path = entry.ok()?.path();
        let name_path = hwmon_path.join("name");
        
        // Read the hwmon device name (coretemp, k10temp, etc.)
        let name = match fs::read_to_string(&name_path) {
            Ok(n) => n.trim().to_lowercase(),
            Err(_) => continue,
        };
        
        // Only process if it's a CPU temperature sensor
        if !name.contains("coretemp") && 
           !name.contains("k10temp") && 
           !name.contains("zenpower") &&
           !name.contains("cpu") {
            continue;
        }
        
        info!("Found CPU hwmon: {}", name);
        
        // Look for temperature labels and inputs
        let mut tdie_path: Option<std::path::PathBuf> = None;
        let mut tctl_path: Option<std::path::PathBuf> = None;
        let mut core_paths: Vec<std::path::PathBuf> = Vec::new();
        
        for i in 0..100 {
            let label_path = hwmon_path.join(format!("temp{}_label", i));
            let input_path = hwmon_path.join(format!("temp{}_input", i));
            
            if !input_path.exists() {
                continue;
            }
            
            if let Ok(label) = fs::read_to_string(&label_path) {
                let label = label.trim();
                
                // Prioritize Tdie > Tctl
                if label.eq_ignore_ascii_case("Tdie") {
                    tdie_path = Some(input_path.clone());
                    info!("  Found Tdie sensor");
                } else if label.eq_ignore_ascii_case("Tctl") {
                    tctl_path = Some(input_path.clone());
                    info!("  Found Tctl sensor");
                } else if label.starts_with("Core") || label.contains("Package") {
                    core_paths.push(input_path.clone());
                    info!("  Found {} sensor", label);
                }
            }
        }
        
        // Use prioritized path: Tdie > Tctl > Core* > Package*
        if let Some(path) = tdie_path.or(tctl_path) {
            if let Some(millideg) = read_temp_millidegrees(&path) {
                return Some(millideg as f32 / 1000.0);
            }
        }
        
        // Fallback to first core/Package sensor
        for path in &core_paths {
            if let Some(millideg) = read_temp_millidegrees(path) {
                return Some(millideg as f32 / 1000.0);
            }
        }
    }

    None
}

/// Read temperature from a millidegree Celsius file and return the raw value.
fn read_temp_millidegrees(path: &std::path::Path) -> Option<i32> {
    use std::fs;
    
    let content = fs::read_to_string(path).ok()?;
    content.trim().parse::<i32>().ok()
}

/// Read the GPU temperature from AMD/Intel DRM hwmon interface.
/// Looks for `/sys/class/drm/card0/device/hwmon/hwmon*/temp1_input` files (values in millidegrees).
fn read_gpu_temperature() -> Option<f32> {
    use std::fs;
    use std::path::PathBuf;

    let drm_path = PathBuf::from("/sys/class/drm/card0/device");

    // Try the hwmon interface first — this is where AMD/Intel expose GPU temperature
    if let Some(hwmon_dir) = find_hwmon_directory(&drm_path) {
        for entry in fs::read_dir(&hwmon_dir).ok()?.flatten() {
            let file_name = entry.file_name();
            // Look for temp1_input, temp2_input, etc. — these are temperature sensor readings
            if let Some(name) = file_name.to_str() {
                if name.starts_with("temp") && name.ends_with("_input") {
                    let content = fs::read_to_string(entry.path()).ok()?;
                    if let Ok(millideg) = content.trim().parse::<u64>() {
                        // Convert millidegrees to degrees Celsius
                        return Some((millideg as f32) / 1000.0);
                    }
                }
            }
        }
    }

    None
}

/// Find the hwmon directory under a DRM device path (e.g., `/sys/class/drm/card0/device/hwmon/hwmon0`).
fn find_hwmon_directory(drm_path: &std::path::Path) -> Option<std::path::PathBuf> {
    use std::fs;

    let hwmon_base = drm_path.join("hwmon");

    for entry in fs::read_dir(&hwmon_base).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // The first hwmon subdirectory is typically the main one (e.g., "hwmon0")
            return Some(path);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Handles None gracefully on unsupported hardware (Beverly writes after implementation).
    #[test]
    fn test_temp_optional_handling() {
        let stats = collect();
        // On systems without thermal sensors, both should be None.
        // On real hardware with CPU/GPU, they may return Some values — just verify no panic occurs.
        if let Some(cpu) = stats.cpu_temp {
            assert!(cpu >= 0.0 && cpu < 200.0, "CPU temperature out of reasonable range: {}°C", cpu);
        }
        if let Some(gpu) = stats.gpu_temp {
            assert!(gpu >= 0.0 && gpu < 300.0, "GPU temperature out of reasonable range: {}°C", gpu);
        }
    }

    /// Verify that the thermal zone reader handles missing directories gracefully.
    #[test]
    fn test_read_cpu_temp_nonexistent() {
        // On systems without /sys/class/thermal (e.g., some containers), this should return None
        let result = find_cpu_temperature();
        // We can't assert it's Some or None — depends on the system. Just verify no panic.
        if let Some(temp) = result {
            assert!(temp >= 0.0 && temp < 200.0);
        }
    }

    /// Verify that GPU temperature reader handles missing hwmon gracefully.
    #[test]
    fn test_read_gpu_temp_nonexistent() {
        let result = read_gpu_temperature();
        // On systems without discrete GPUs, this should return None
        if let Some(temp) = result {
            assert!(temp >= 0.0 && temp < 300.0);
        }
    }
}
