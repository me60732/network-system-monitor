//! AMD GPU statistics via sysfs (DRM).

use std::fs;
use log::{debug, warn};

use super::GpuStats;

/// Collect AMD GPU stats from sysfs.
///
/// Returns Some(GpuStats) if AMD GPU detected, None otherwise.
pub fn collect() -> Option<GpuStats> {
    let cards = find_amd_cards();
    
    if cards.is_empty() {
        return None;
    }
    
    // Use first AMD card found
    let card = &cards[0];
    debug!("Found AMD GPU: {}", card);
    
    let base = format!("/sys/class/drm/{}/device", card);
    
    // Read VRAM total and used
    let vram_total = read_sysfs_u64(&format!("{}/mem_info_vram_total", base));
    let vram_used = read_sysfs_u64(&format!("{}/mem_info_vram_used", base));
    
    // Read GPU utilization (0-100 integer)
    let gpu_load_percent = read_sysfs_u64(&format!("{}/gpu_busy_percent", base))
        .map(|v| v as f32);
    
    // Read GPU temperature from hwmon (millidegrees Celsius)
    let gpu_temp = find_and_read_temperature(card);
    
    debug!("AMD GPU stats: vram_used={:?}, vram_total={:?}, load={:?}%, temp={:?}°C", 
           vram_used.map(|v| v / 1_048_576), vram_total.map(|v| v / 1_048_576), gpu_load_percent, gpu_temp);
    
    Some(GpuStats {
        vram_total,
        vram_used,
        gpu_load_percent,
        gpu_temp,
    })
}

/// Find all AMD GPU cards by scanning /sys/class/drm for vendor ID 0x1002.
fn find_amd_cards() -> Vec<String> {
    let mut cards = Vec::new();
    
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return cards;
    };
    
    for entry in entries.flatten() {
        let path = entry.path();
        let vendor_path = path.join("device/vendor");
        
        if !vendor_path.exists() {
            continue;
        }
        
        let Ok(vendor_id) = fs::read_to_string(&vendor_path) else {
            continue;
        };
        
        if vendor_id.trim() == "0x1002" {
            if let Some(card) = path.file_name().and_then(|n| n.to_str()) {
                // Only physical cards (not virtual connectors like card0-DP-1)
                if card.starts_with("card") && !card.contains('-') {
                    debug!("Found AMD card: {}", card);
                    cards.push(card.to_string());
                }
            }
        }
    }
    
    cards
}

/// Find and read GPU temperature from hwmon (returns Celsius).
fn find_and_read_temperature(card: &str) -> Option<f32> {
    let hwmon_base = format!("/sys/class/drm/{}/device/hwmon", card);
    
    let Ok(entries) = fs::read_dir(&hwmon_base) else {
        return None;
    };
    
    for entry in entries.flatten() {
        let temp_path = entry.path().join("temp1_input");
        if temp_path.exists() {
            if let Ok(content) = fs::read_to_string(&temp_path) {
                if let Ok(millideg) = content.trim().parse::<u64>() {
                    // Convert millidegrees to Celsius
                    return Some((millideg as f32) / 1000.0);
                }
            }
        }
    }
    
    None
}

/// Read a sysfs file and parse as u64.
fn read_sysfs_u64(path: &str) -> Option<u64> {
    let content = fs::read_to_string(path).ok()?;
    content.trim().parse::<u64>().ok()
}
