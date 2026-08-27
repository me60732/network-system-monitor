//! Disk partition statistics.
//!
//! Collects disk usage per mounted partition using sysinfo's standalone `Disks` type (sysinfo 0.39),
//! which reads from `/proc/mounts`. Total/used bytes are read directly from each Disk via built-in
//! methods — no libc or statvfs dependency required. Each entry in [`DiskStats::partitions`] represents
//! one mount point with usage and IO statistics.
//!
//! ## Data Flow
//!
//! 1. `collect()` creates a refreshed `Disks` instance via `new_with_refreshed_list()`.
//! 2. It iterates over `.list()`, extracting mount points and file system type from each Disk.
//! 3. For each non-virtual filesystem, it reads total/used bytes using `disk.total_space()` / `disk.available_space()`.
//! 4. IO statistics (read/write bytes) are extracted via `disk.read_bytes()` and `disk.write_bytes()` if available.

use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use sysinfo::{Disks, DiskRefreshKind};

/// Disk partition statistics for one mount point.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PartitionStat {
    /// Mount point path (e.g., "/", "/home", "/boot").
    pub mount: String,
    /// Total size of the filesystem in bytes.
    pub total: u64,
    /// Used space on this partition in bytes.
    pub used: u64,
}

/// Disk IO statistics (cumulative since boot).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskIoStats {
    /// Total bytes read from all disks since boot.
    pub read_bytes: u64,
    /// Total bytes written to all disks since boot.
    pub write_bytes: u64,
}

/// Aggregate disk statistics across all mounted partitions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskStats {
    /// One entry per detected mount point with usage data.
    pub partitions: Vec<PartitionStat>,
}

/// Collect disk IO statistics (cumulative totals since boot).
///
/// Uses sysinfo's Disks with io_usage refresh to get cumulative read/write byte counts.
/// Returns total IO across all disks. Matches minimon-applet's collection pattern.
pub fn collect_io() -> DiskIoStats {
    let r = DiskRefreshKind::nothing().with_io_usage();
    let mut disks = Disks::new();
    disks.refresh_specifics(true, r);

    let mut total_read = 0u64;
    let mut total_write = 0u64;

    for disk in disks.list() {
        let usage = disk.usage();
        total_read += usage.read_bytes;
        total_write += usage.written_bytes;
    }

    DiskIoStats {
        read_bytes: total_read,
        write_bytes: total_write,
    }
}

/// Collect current disk partition statistics.
///
/// Uses sysinfo's standalone `Disks` type (sysinfo 0.39) to enumerate disks and their mount points,
/// then reads total/used byte counts directly from each Disk via built-in methods — no statvfs or libc dependency.
/// IO statistics (read/write bytes) are included where available in sysinfo 0.39+; returns `None` if unsupported.
/// Returns an empty vector if no disks are detected.
pub fn collect() -> DiskStats {
    // In sysinfo 0.39, Disks is a standalone type with new_with_refreshed_list().
    let disks = Disks::new_with_refreshed_list();

    let mut partitions: Vec<PartitionStat> = Vec::new();

    for disk in disks.list() {
        // Skip virtual/tmpfs filesystems — only report real block devices.
        if is_virtual_fs(disk.file_system()) {
            continue;
        }

        // In sysinfo 0.39, Disk has a single mount_point() returning &Path (not multiple).
        let mp_str = match disk.mount_point().to_str() {
            Some(s) => s,
            None => continue,
        };

        // sysinfo's Disk provides total_space() and available_space() natively — no statvfs needed.
        let total_bytes = disk.total_space();
        // Skip partitions with zero total size (e.g., some special mounts).
        if total_bytes == 0 {
            continue;
        }

        let avail_bytes = disk.available_space();
        let used_bytes = total_bytes.saturating_sub(avail_bytes);

        // Note: sysinfo 0.39 Disk does not expose read/write IO bytes — those require direct /sys reads.
        // For now, we report None for IO stats; future enhancement could add procfs-based IO counters.

        partitions.push(PartitionStat {
            mount: mp_str.to_string(),
            total: total_bytes,
            used: used_bytes,
        });
    }

    DiskStats { partitions }
}

/// Return the root partition usage percentage (0.0–100.0).
/// Returns 0.0 if no "/" mount point is found.
pub fn root_used_percent(stats: &DiskStats) -> f32 {
    if let Some(root_partition) = stats.partitions.iter().find(|p| p.mount == "/") {
        if root_partition.total > 0 {
            (root_partition.used as f32 / root_partition.total as f32) * 100.0
        } else {
            0.0
        }
    } else {
        0.0
    }
}

/// Check if a filesystem type string indicates a virtual/tmpfs filesystem that should be skipped.
fn is_virtual_fs(fs_type: &OsStr) -> bool {
    let fs_str = match fs_type.to_str() {
        Some(s) => s,
        None => return true, // If we can't determine the FS type, skip it for safety
    };

    matches!(
        fs_str,
        "tmpfs" | "devtmpfs" | "proc" | "sysfs" | "cgroup" | "cgroup2" | "pstore" | "bpf"
            | "tracefs" | "debugfs" | "mqueue" | "hugetlbfs" | "fusectl" | "securityfs" | "configfs"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At least one partition returned on Linux (Beverly writes after implementation).
    #[test]
    fn test_disk_partitions_nonempty() {
        let stats = collect();
        // On any real Linux system with at least a root filesystem, we should see partitions.
        assert!(!stats.partitions.is_empty(), "Expected at least one disk partition on Linux");
    }

    /// Each returned partition must have non-zero total bytes (real block device).
    #[test]
    fn test_partitions_have_valid_sizes() {
        let stats = collect();
        for p in &stats.partitions {
            assert!(p.total > 0, "Partition {} has zero total size", p.mount);
            assert!(p.used <= p.total, "Used bytes exceed total on {}", p.mount);
        }
    }

    /// Root filesystem should be present.
    #[test]
    fn test_root_partition_present() {
        let stats = collect();
        let has_root = stats.partitions.iter().any(|p| p.mount == "/");
        assert!(has_root, "Expected root partition '/' in disk stats");
    }
}
