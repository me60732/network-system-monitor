//! System temperature statistics.
//!
/// Collects CPU and GPU temperatures in Celsius using `/sys/class/thermal/` on Linux.
/// On unsupported hardware, fields return `None`.

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
/// Reads CPU temperature from `/sys/class/thermal/thermal_zone*/temp` files (values are in millidegrees Celsius).
/// Attempts to read GPU temperature from `/sys/class/drm/card0/device/hwmon/hwmon*/temp1_input` if available.
/// Returns `None` for any sensor that cannot be read, rather than panicking — this allows the system
/// to gracefully degrade on hardware without thermal sensors (e.g., some virtual machines or containers).
pub fn collect() -> TemperatureStats {
    let cpu_temp = read_cpu_temperature();
    let gpu_temp = read_gpu_temperature();

    TemperatureStats {
        cpu_temp,
        gpu_temp,
    }
}

/// Read the CPU temperature from `/sys/class/thermal/` thermal zones.
/// Iterates over all `thermal_zone*` directories and returns the first valid temperature in Celsius.
fn read_cpu_temperature() -> Option<f32> {
    use std::fs;
    use std::path::PathBuf;

    let base = PathBuf::from("/sys/class/thermal");

    // Read directory entries for thermal_zone* directories
    let entries = match fs::read_dir(&base) {
        Ok(entries) => entries,
        Err(_) => return None,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Check if the directory name starts with "thermal_zone"
        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
            if dir_name.starts_with("thermal_zone") {
                // Read the temp file — value is in millidegrees Celsius
                let temp_path = path.join("temp");
                if let Ok(content) = fs::read_to_string(&temp_path) {
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
        let result = read_cpu_temperature();
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
