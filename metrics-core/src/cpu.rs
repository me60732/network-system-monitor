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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoreStat {
    /// Zero-indexed core number (0 = first logical processor).
    pub index: u32,
    /// Current usage percentage for this core (0.0–100.0).
    pub usage: f32,
}

/// Aggregate CPU statistics across all cores.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuStats {
    /// Overall system-wide CPU usage percentage (0.0–100.0).
    pub usage: f32,
    /// Per-core breakdown — one entry per logical processor.
    pub cores: Vec<CoreStat>,
}

/// Raw CPU time statistics from /proc/stat parsing.
///
/// Each field represents accumulated CPU time in clock ticks since boot:
/// - user: normal processes executing in user mode
/// - nice: niced processes executing in user mode  
/// - system: processes executing in kernel mode
/// - idle: waiting for I/O or no work to do
/// - iowait: waiting for I/O (but idle otherwise)
/// - irq: servicing hardware interrupts
/// - softirq: servicing software interrupts
/// - steal: time spent in other OSes when virtualized
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
}

impl CpuCollector {
    /// Create a new CPU collector and prime state with initial /proc/stat read.
    ///
    /// Performs one initial read of `/proc/stat` to populate `current_stats`,
    /// then clones it into `prev_stats`. The first call to `collect()` will
    /// compare this initial state against the next read, yielding accurate deltas.
    pub fn new() -> Self {
        let mut collector = Self {
            prev_stats: HashMap::new(),
            current_stats: HashMap::new(),
            cores: Vec::new(),  // Initialize empty per-core stats storage
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
        
        CpuStats { usage, cores: std::mem::take(&mut self.cores) }
    }
}

impl Default for CpuCollector {
    fn default() -> Self {
        Self::new()
    }
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
        
        // First collect should compute deltas from the initial baseline
        let stats1 = collector.collect();
        assert!(stats1.usage >= 0.0 && stats1.usage <= 100.0);
        assert!(!stats1.cores.is_empty());
        
        // Second collect should use the previous result as baseline
        let stats2 = collector.collect();
        assert!(stats2.usage >= 0.0 && stats2.usage <= 100.0);
        assert!(!stats2.cores.is_empty());
    }
}
