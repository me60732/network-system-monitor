//! CPU usage statistics.
//!
//! Collects aggregate and per-core CPU utilization by reading `/proc/stat` directly on Linux.
//! Usage is expressed as a percentage (0.0–100.0).
//!
//! ## State Management Pattern
//!
//! **Critical**: CPU percentage requires TWO measurements with time delta to compute accurate utilization.
//! - Single snapshot returns 0.0 or stale value (no delta = no percentage)
//! - Use [`CpuCollector`] with state management for accurate delta measurement
//! - Initialize once: `let mut collector = CpuCollector::new()`
//! - Call repeatedly: `let stats = collector.collect()` (calculates current - previous)
//!
//! ## Data Flow
//!
//! 1. Read `/proc/stat` line by line, parse each cpuN entry
//! 2. Store as [`CpuStat`] with user/nice/system/idle/iowait/irq/softirq/steal fields
//! 3. Maintain two HashMaps: `prev_stats` and `current_stats`
//! 4. On collect(): read current, calculate deltas (current - prev), store current as new prev
//! 5. Compute percentages from deltas: `(user + nice) / total * 100.0`
//!
//! ## Performance Target
//!
//! Full collection (all modules) must complete in < 50ms for real-time panel updates.
//! CPU delta measurement adds ~1-2ms overhead for the extra /proc/stat read.
#![allow(missing_docs)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-core CPU utilization statistics.
///
/// Represents the CPU usage for a single logical processor core.
/// - `index`: Zero-indexed core number (0 = first logical processor)
/// - `usage`: Current usage percentage for this core (0.0–100.0)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoreStat {
    pub index: u32,
    pub usage: f32,
}

/// Aggregate CPU statistics across all cores.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuStats {
    /// Overall system-wide CPU usage percentage (0.0–100.0).
    pub usage: f32,
    /// Per-core breakdown — one entry per logical processor.
    pub cores: Vec<CoreStat>,
    /// CPU package temperature in Celsius, or `None` if unavailable.
    pub cpu_temp: Option<f32>,
}

/// Raw CPU time statistics from /proc/stat parsing.
///
/// Each field represents accumulated CPU time in clock ticks since boot:
/// - `user`: normal processes executing in user mode
/// - `nice`: niced processes executing in user mode  
/// - `system`: processes executing in kernel mode
/// - `idle`: waiting for I/O or no work to do
/// - `iowait`: waiting for I/O (but idle otherwise)
/// - `irq`: servicing hardware interrupts
/// - `softirq`: servicing software interrupts
/// - `steal`: time spent in other OSes when virtualized
#[derive(Debug, Clone, Copy)]
pub struct CpuStat {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

/// Stateful CPU collector for accurate delta measurement.
///
/// Maintains previous and current CPU statistics to compute utilization percentages
/// via the delta method. This is the only way to get accurate CPU usage on Linux
/// since `/proc/stat` provides cumulative counters, not instantaneous percentages.
///
/// ## Usage Pattern
///
/// ```no_run
/// use metrics_core::CpuCollector;
/// 
/// let mut collector = CpuCollector::new();
/// 
/// // Prime state with initial read (done in new())
/// 
/// // Collect metrics repeatedly - each call calculates current - previous
/// loop {
///     let stats = collector.collect();
///     println!("CPU usage: {:.1}%", stats.usage);
///     
///     // Sleep between collections (recommended: 500ms-2s for service)
///     std::thread::sleep(std::time::Duration::from_secs(1));
/// }
/// ```
///
/// ## Why State Management?
///
/// - `/proc/stat` shows cumulative ticks since boot, not percentages
/// - Percentage = (delta_user + delta_nice) / delta_total * 100.0
/// - Without two measurements, you cannot compute deltas → 0.0 or stale value
/// - sysinfo's single-snapshot approach returns inaccurate values for this reason
pub struct CpuCollector {
    prev_stats: HashMap<usize, CpuStat>,
    current_stats: HashMap<usize, CpuStat>,
    /// Per-core CPU load storage (transient, cleared each collect())
    cores: Vec<CoreStat>,
    /// Cached CPU temperature sensor path (discovered once at initialization)
    cpu_sensor_path: Option<std::path::PathBuf>,
}

impl CpuCollector {
    /// Create a new CPU collector and prime state with initial /proc/stat read.
    ///
    /// Performs one initial read of `/proc/stat` to populate `current_stats`,
    /// then clones it into `prev_stats`. The first call to `collect()` will
    /// compare this initial state against the next read, yielding accurate deltas.
    /// 
    /// Also discovers CPU temperature sensor path once at initialization.
    pub fn new() -> Self {
        let cpu_sensor_path = find_cpu_sensor_path();
        
        if let Some(ref path) = cpu_sensor_path {
            log::info!("CpuCollector: Found CPU sensor at {:?}", path);
        } else {
            log::warn!("CpuCollector: No CPU temperature sensor found");
        }
        
        let mut collector = Self {
            prev_stats: HashMap::new(),
            current_stats: HashMap::new(),
            cores: Vec::new(),  // Initialize empty per-core stats storage
            cpu_sensor_path,
        };
        
        // Prime with initial read - both maps get the same data initially
        CpuCollector::read_cpu_stats(&mut collector.current_stats);
        collector.prev_stats = collector.current_stats.clone();
        
        collector
    }
    
    /// Read /proc/stat and parse all CPU entries into the provided HashMap.
    ///
    /// Format: "cpu0 123456 789 456 78901 123 45 67 8"
    /// Fields: user nice system idle iowait irq softirq steal
    fn read_cpu_stats(cpu_stats: &mut HashMap<usize, CpuStat>) {
        let content = match std::fs::read_to_string("/proc/stat") {
            Ok(c) => c,
            Err(_) => return,  // Silently fail if /proc/stat is unreadable
        };
        
        for line in content.lines() {
            // Skip non-CPU lines
            if !line.starts_with("cpu") || line == "cpu" {
                continue;
            }
            
            let parts: Vec<&str> = line.split_whitespace().collect();
            
            // Need at least 9 fields: cpuN + 8 time values
            if parts.len() < 9 {
                continue;
            }
            
            // Extract core number from "cpu0", "cpu1", etc.
            let core_num = match parts[0].trim_start_matches("cpu").parse::<usize>() {
                Ok(n) => n,
                Err(_) => continue,
            };
            
            // Parse all time values (default to 0 if parse fails)
            let user = parts[1].parse::<u64>().unwrap_or(0);
            let nice = parts[2].parse::<u64>().unwrap_or(0);
            let system = parts[3].parse::<u64>().unwrap_or(0);
            let idle = parts[4].parse::<u64>().unwrap_or(0);
            let iowait = parts[5].parse::<u64>().unwrap_or(0);
            let irq = parts[6].parse::<u64>().unwrap_or(0);
            let softirq = parts[7].parse::<u64>().unwrap_or(0);
            let steal = parts[8].parse::<u64>().unwrap_or(0);
            
            cpu_stats.insert(core_num, CpuStat {
                user, nice, system, idle, iowait, irq, softirq, steal,
            });
        }
    }
    
    /// Collect current CPU statistics by computing delta from previous state.
    ///
    /// 1. Read current /proc/stat into a temporary HashMap
    /// 2. For each core: compute deltas (current - prev) for all time fields
    /// 3. Calculate percentage: `(delta_user + delta_nice) / delta_total * 100.0`
    /// 4. Accumulate across all cores to get system-wide usage
    /// 5. Store current as new prev for next cycle
    ///
    /// Returns aggregate CPU usage (0.0–100.0) and per-core breakdown.
    pub fn collect(&mut self) -> CpuStats {
        // Read fresh current stats into a temporary HashMap first
        let mut temp_stats = HashMap::new();
        Self::read_cpu_stats(&mut temp_stats);
        
        // Running totals for average computation across all cores
        let mut total_user_pct = 0.0;
        let mut total_system_pct = 0.0;
        let mut counted_cores = 0;
        
        self.cores.clear();
        
        for (&core_num, current) in &temp_stats {
            if let Some(prev) = self.prev_stats.get(&core_num) {
                // Compute time deltas (saturating_sub prevents underflow on counter wrap)
                let user = current.user.saturating_sub(prev.user);
                let nice = current.nice.saturating_sub(prev.nice);
                let system = current.system.saturating_sub(prev.system);
                let idle = current.idle.saturating_sub(prev.idle);
                let iowait = current.iowait.saturating_sub(prev.iowait);
                let irq = current.irq.saturating_sub(prev.irq);
                let softirq = current.softirq.saturating_sub(prev.softirq);
                let steal = current.steal.saturating_sub(prev.steal);
                
                // Total delta time across all states
                let total = user + nice + system + idle + iowait + irq + softirq + steal;
                
                if total == 0 {
                    continue; // No activity recorded, skip this core
                }
                
                // Calculate percentages from deltas
                let total_f64 = total as f64;
                let user_pct = (user + nice) as f64 / total_f64 * 100.0;
                let system_pct = system as f64 / total_f64 * 100.0;
                
                // Store per-core stats
                self.cores.push(CoreStat {
                    index: core_num as u32,
                    usage: (user_pct + system_pct) as f32,
                });
                
                // Accumulate for aggregate calculation
                total_user_pct += user_pct;
                total_system_pct += system_pct;
                counted_cores += 1;
                
                // Update prev for next cycle (current becomes new baseline)
                self.prev_stats.insert(core_num, *current);
            }
        }
        
        // Compute aggregate across all cores that contributed data
        let usage = if counted_cores > 0 {
            let core_count_f64 = counted_cores as f64;
            ((total_user_pct + total_system_pct) / core_count_f64) as f32
        } else {
            0.0 // No cores found or all had zero delta
        };
        
        // Swap temp_stats into current_stats for next iteration baseline
        std::mem::swap(&mut self.current_stats, &mut temp_stats);
        
        // Read CPU temperature from cached sensor path
        let cpu_temp = self.cpu_sensor_path.as_ref()
            .and_then(|path| read_temperature_from_path(path));
        
        CpuStats { 
            usage, 
            cores: std::mem::take(&mut self.cores),
            cpu_temp,
        }
    }
}

impl Default for CpuCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Find CPU temperature sensor path by scanning /sys/class/hwmon/*.
///
/// Returns the path to the highest-priority temperature sensor input file:
/// - **AMD**: Tdie > Tctl > Core* > Package*
/// - **Intel**: Core* > Package*
///
/// This function performs expensive hwmon directory scanning but only needs to be called once
/// at initialization. The resulting path is cached by CpuCollector for fast reads.
fn find_cpu_sensor_path() -> Option<std::path::PathBuf> {
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
        
        log::info!("Found CPU hwmon: {}", name);
        
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
                    log::info!("  Found Tdie sensor");
                } else if label.eq_ignore_ascii_case("Tctl") {
                    tctl_path = Some(input_path.clone());
                    log::info!("  Found Tctl sensor");
                } else if label.starts_with("Core") || label.contains("Package") {
                    core_paths.push(input_path.clone());
                    log::info!("  Found {} sensor", label);
                }
            }
        }
        
        // Use prioritized path: Tdie > Tctl > Core* > Package*
        if let Some(path) = tdie_path.or(tctl_path) {
            return Some(path);
        }
        
        // Fallback to first core/Package sensor
        if let Some(path) = core_paths.first() {
            return Some(path.clone());
        }
    }

    None
}

/// Read temperature value from a sensor path (e.g., /sys/class/hwmon/hwmon0/temp1_input).
/// Returns temperature in Celsius, or None if read fails.
fn read_temperature_from_path(path: &std::path::Path) -> Option<f32> {
    use std::fs;
    let raw = fs::read_to_string(path).ok()?;
    let millidegrees: i32 = raw.trim().parse().ok()?;
    Some(millidegrees as f32 / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CpuCollector creates with empty maps and primes state on initialization.
    #[test]
    fn test_collector_initialization() {
        let collector = CpuCollector::new();
        // After new(), both prev_stats and current_stats should be populated
        assert!(!collector.prev_stats.is_empty());
        assert!(!collector.current_stats.is_empty());
        // cores should be empty initially
        assert!(collector.cores.is_empty());
    }
    
    /// CpuCollector can collect CPU stats multiple times with state persistence.
    #[test]
    fn test_collector_collect_multiple_times() {
        let mut collector = CpuCollector::new();
        
        // Sleep briefly to ensure CPU activity registers between priming and first collect
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // First collect should compute deltas from the initial baseline
        let stats1 = collector.collect();
        assert!(stats1.usage >= 0.0 && stats1.usage <= 100.0);
        assert!(!stats1.cores.is_empty());
        
        // Sleep before second collect
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Second collect should use the previous result as baseline
        let stats2 = collector.collect();
        assert!(stats2.usage >= 0.0 && stats2.usage <= 100.0);
        assert!(!stats2.cores.is_empty());
    }
}
