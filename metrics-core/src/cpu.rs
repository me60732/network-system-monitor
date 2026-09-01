//! CPU usage statistics.
//!
//! Collects aggregate and per-core CPU utilization using `procfs` on Linux.
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
//! 1. Read CPU time via `procfs::KernelStats::current()` — per-core Vec with total aggregate
//! 2. Store as `Vec<procfs::CpuTime>` with user/nice/system/idle/iowait/irq/softirq/steal fields
//! 3. Maintain two Vecs: `prev_stats` and `current_stats`
//! 4. On collect(): read current, calculate deltas (current - prev), store current as new prev
//! 5. Compute percentages from deltas: `(user + nice) / total * 100.0`
//!
//! ## Performance Target
//!
//! Full collection (all modules) must complete in < 50ms for real-time panel updates.
//! CPU delta measurement adds ~1-2ms overhead for the extra procfs read.
#![allow(missing_docs)]
use procfs::CurrentSI;
/// Per-core CPU utilization statistics.
///
/// Represents the CPU usage for a single logical processor core.
/// - `index`: Zero-indexed core number (0 = first logical processor)
/// - `usage`: Current usage percentage for this core (0.0–100.0)
#[derive(Debug, Clone, Default)]
pub struct CoreStat {
    pub index: u32,
    pub usage: f32,
}

/// Aggregate CPU statistics across all cores.
#[derive(Debug, Clone, Default)]
pub struct CpuStats {
    /// Overall system-wide CPU usage percentage (0.0–100.0).
    pub usage: f32,
    /// Per-core breakdown — one entry per logical processor.
    pub cores: Vec<CoreStat>,
    /// CPU package temperature in Celsius, or `None` if unavailable.
    pub cpu_temp: Option<f32>,
}

/// Stateful CPU collector for accurate delta measurement.
///
/// Maintains previous and current CPU statistics to compute utilization percentages
/// via the delta method. This is the only way to get accurate CPU usage on Linux
/// since procfs provides cumulative counters, not instantaneous percentages.
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
/// - procfs shows cumulative ticks since boot, not percentages
/// - Percentage = (delta_user + delta_nice) / delta_total * 100.0
/// - Without two measurements, you cannot compute deltas → 0.0 or stale value
/// - sysinfo's single-snapshot approach returns inaccurate values for this reason
pub struct CpuCollector {
    prev_stats: Vec<procfs::CpuTime>,
    current_stats: Vec<procfs::CpuTime>,
    /// Per-core CPU load storage (transient, cleared each collect())
    cores: Vec<CoreStat>,
    /// Cached CPU temperature sensor path (discovered once at initialization)
    cpu_sensor_path: Option<std::path::PathBuf>,
}

impl CpuCollector {
    /// Create a new CPU collector and prime state with initial procfs read.
    ///
    /// Performs one initial read via `procfs::KernelStats::current()` to populate `current_stats`,
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
            prev_stats: Vec::new(),
            current_stats: Vec::new(),
            cores: Vec::new(), // Initialize empty per-core stats storage
            cpu_sensor_path,
        };

        // Prime with initial read - both vecs get the same data initially
        CpuCollector::read_cpu_stats(&mut collector.current_stats);
        collector.prev_stats = collector.current_stats.clone();

        collector
    }

    /// Read CPU statistics via procfs.
    ///
    /// Returns a Vec of CpuTime entries, one per core (index 0 = cpu0).
    fn read_cpu_stats(cpu_stats: &mut Vec<procfs::CpuTime>) {
        *cpu_stats = match procfs::KernelStats::current() {
            Ok(stats) => stats.cpu_time,
            Err(_) => Vec::new(),
        };
    }

    /// Collect current CPU statistics by computing delta from previous state.
    ///
    /// 1. Read fresh current stats via procfs into a temporary Vec
    /// 2. For each core: compute deltas (current - prev) for all time fields
    /// 3. Calculate percentage: `(delta_user + delta_nice) / delta_total * 100.0`
    /// 4. Accumulate across all cores to get system-wide usage
    /// 5. Store current as new prev for next cycle
    ///
    /// Returns aggregate CPU usage (0.0–100.0) and per-core breakdown.
    pub fn collect(&mut self) -> CpuStats {
        // Read fresh current stats into a temporary Vec first
        let mut temp_stats = Vec::new();
        Self::read_cpu_stats(&mut temp_stats);

        // Running totals for average computation across all cores
        let mut total_user_pct = 0.0;
        let mut total_system_pct = 0.0;
        let mut counted_cores = 0;

        self.cores.clear();

        for (core_num, current) in temp_stats.iter().enumerate() {
            if core_num == 0 {
                // Skip the aggregate "cpu" line at index 0
                continue;
            }

            if let Some(prev) = self.prev_stats.get(core_num) {
                // Compute time deltas (saturating_sub prevents underflow on counter wrap)
                let user = current.user.saturating_sub(prev.user);
                let nice = current.nice.saturating_sub(prev.nice);
                let system = current.system.saturating_sub(prev.system);
                let idle = current.idle.saturating_sub(prev.idle);
                let iowait = current
                    .iowait
                    .unwrap_or(0)
                    .saturating_sub(prev.iowait.unwrap_or(0));
                let irq = current
                    .irq
                    .unwrap_or(0)
                    .saturating_sub(prev.irq.unwrap_or(0));
                let softirq = current
                    .softirq
                    .unwrap_or(0)
                    .saturating_sub(prev.softirq.unwrap_or(0));
                let steal = current
                    .steal
                    .unwrap_or(0)
                    .saturating_sub(prev.steal.unwrap_or(0));

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
                self.prev_stats[core_num] = current.clone();
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
        let cpu_temp = self
            .cpu_sensor_path
            .as_ref()
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

/// Cross-architecture thermal zone scanner.
///
/// Scans /sys/class/thermal/thermal_zone*/ for CPU temperature sensors.
/// Prioritizes sensors by type: x86_pkg_temp > cpu-thermal/cpu0 > cpu > acpitz > generic.
fn find_cpu_sensor_path() -> Option<std::path::PathBuf> {
    let base = std::path::Path::new("/sys/class/thermal");
    let mut best: Option<(usize, std::path::PathBuf)> = None;

    let entries = match std::fs::read_dir(base) {
        Ok(e) => e,
        Err(_) => return None,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.starts_with("thermal_zone") {
            continue;
        }
        let zone_num: usize = match name["thermal_zone".len()..].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let temp_path = path.join("temp");
        if !temp_path.exists() {
            continue;
        }

        // Read zone type to determine priority (lower score = better)
        let zone_type = std::fs::read_to_string(path.join("type"))
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default();

        // Priority: CPU-package sensors first, then generic, then by zone number
        let type_priority: usize = if zone_type.contains("x86_pkg_temp") {
            0
        } else if zone_type.contains("cpu-thermal") || zone_type.contains("cpu0") {
            1
        } else if zone_type.contains("cpu") {
            2
        } else if zone_type.contains("acpitz") {
            3
        } else {
            4
        };

        let score = type_priority * 1000 + zone_num;

        if best.as_ref().map_or(true, |(s, _)| score < *s) {
            best = Some((score, temp_path));
        }
    }

    if let Some((_, ref path)) = best {
        log::info!("CPU temp sensor: {:?}", path);
    } else {
        log::warn!("No CPU thermal zone found under /sys/class/thermal/");
    }

    best.map(|(_, p)| p)
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

    /// CpuCollector creates with empty vecs and primes state on initialization.
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
