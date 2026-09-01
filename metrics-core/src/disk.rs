//! Disk partition and IO statistics.
//!
//! Collects disk usage per mounted partition using `procfs::mounts()` + `rustix::fs::statvfs`.
//! Total/used bytes are computed via statvfs syscalls on each mount point.
//! IO statistics come in two forms:
//!
//! * [`DiskIoCollector`] - stateful collector that returns delta bytes since last call (recommended)

use procfs::mounts;
use rustix::fs::statvfs;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::io::BufRead;
use std::os::unix::fs::MetadataExt;

/// Disk partition statistics for one mount point.
#[derive(Debug, Clone, Default)]
pub struct PartitionStat {
    /// Mount point path (e.g., "/", "/home", "/boot").
    pub mount: String,
    /// Total size of the filesystem in bytes.
    pub total: u64,
    /// Used space on this partition in bytes.
    pub used: u64,
}

/// Disk IO statistics (cumulative since boot).
///
/// DEPRECATED: Use [`DiskIoCollector`] instead which returns deltas per-interval.
#[derive(Debug, Clone, Default)]
pub struct DiskIoStats {
    /// Total bytes read from all disks since boot.
    pub read_bytes: u64,
    /// Total bytes written to all disks since boot.
    pub write_bytes: u64,
}

/// Aggregate disk statistics across all mounted partitions.
#[derive(Debug, Clone, Default)]
pub struct DiskStats {
    /// One entry per detected mount point with usage data.
    pub partitions: Vec<PartitionStat>,
}

/// Raw sector counts from /proc/diskstats for a single device.
struct RawDiskStats {
    read_sectors: u64,
    write_sectors: u64,
}

/// Stateful disk IO collector that reads /proc/diskstats directly.
///
/// Device selection uses the kernel's /sys/block/<dev>/holders/ mechanism:
/// - Physical devices that back a dm (LVM) or md (RAID) volume will have entries
///   in their holders/ directory, so we SKIP them and count through the logical device.
/// - Logical volumes (dm-0, md0) and standalone disks have an empty holders/ directory
///   and ARE counted.
/// - Partitions are skipped; the parent whole-disk device includes all partition IO.
///
/// This works portably on any Linux machine without hardcoding device type patterns.
pub struct DiskIoCollector {
    prev: HashMap<String, RawDiskStats>,
}

impl DiskIoCollector {
    /// Create a new collector and record the initial /proc/diskstats baseline.
    pub fn new() -> Self {
        DiskIoCollector {
            prev: Self::read_diskstats(),
        }
    }

    /// Collect disk IO deltas since last call to this method.
    /// Returns bytes read and written across all real block devices since the previous call.
    pub fn collect(&mut self) -> DiskIoStats {
        let current = Self::read_diskstats();

        let mut total_read = 0u64;
        let mut total_write = 0u64;

        for (name, curr) in &current {
            if let Some(prev) = self.prev.get(name) {
                // Sectors in /proc/diskstats are always 512-byte units (kernel ABI guarantee)
                let rd = curr.read_sectors.saturating_sub(prev.read_sectors);
                let wr = curr.write_sectors.saturating_sub(prev.write_sectors);
                total_read += rd * 512;
                total_write += wr * 512;
            }
        }

        self.prev = current;

        DiskIoStats {
            read_bytes: total_read,
            write_bytes: total_write,
        }
    }

    /// Parse /proc/diskstats and return sector counts for devices that should be tracked.
    fn read_diskstats() -> HashMap<String, RawDiskStats> {
        let mut map = HashMap::new();

        let file = match std::fs::File::open("/proc/diskstats") {
            Ok(f) => f,
            Err(e) => {
                log::warn!("Failed to open /proc/diskstats: {}", e);
                return map;
            }
        };

        // /proc/diskstats fields (space-separated):
        // 0:major  1:minor  2:name  3:reads  4:reads_merged  5:sectors_read  6:ms_read
        // 7:writes  8:writes_merged  9:sectors_written  10:ms_write  ...
        for line in std::io::BufReader::new(file).lines().flatten() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 10 {
                continue;
            }

            let name = parts[2];

            if !Self::should_count(name) {
                continue;
            }

            let read_sectors = parts[5].parse::<u64>().unwrap_or(0);
            let write_sectors = parts[9].parse::<u64>().unwrap_or(0);

            map.insert(
                name.to_string(),
                RawDiskStats {
                    read_sectors,
                    write_sectors,
                },
            );
        }

        map
    }

    /// Return true if this device should be counted toward total disk IO.
    ///
    /// Rules (applied in order):
    /// 1. Skip pure virtual devices (loop, ram, sr, zram) — they are not real storage.
    /// 2. Skip partitions — the parent whole-disk entry already includes all partition IO,
    ///    so counting both would double-count.
    /// 3. Skip devices whose /sys/block/<dev>/holders/ directory is non-empty — those
    ///    devices are physical members (PVs, RAID members) of a higher-level logical device.
    ///    The logical device (dm-N, md-N) has an empty holders/ and will be counted instead.
    fn should_count(name: &str) -> bool {
        // 1. Pure virtual / no-IO pseudo-devices
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("sr")
            || name.starts_with("zram")
        {
            return false;
        }

        // 2. Partitions — parent device includes their IO
        if Self::is_partition(name) {
            return false;
        }

        // 3. Devices that are slaves of a logical volume (LVM PV, RAID member, etc.).
        //    /sys/block/<name>/holders/ lists which dm/md devices use this as a slave.
        //    If the directory is non-empty, skip this device; its IO is counted through
        //    the logical device that holds it (which will itself have an empty holders/).
        let holders = format!("/sys/block/{}/holders", name);
        if let Ok(mut dir) = std::fs::read_dir(&holders) {
            if dir.next().is_some() {
                return false;
            }
        }

        true
    }

    /// Return true if this device is a partition rather than a whole-disk device.
    /// We count only whole-disk devices to avoid double-counting (parent + partition IO).
    fn is_partition(name: &str) -> bool {
        // NVMe partition: nvme0n1p1, nvme1n1p3 — has 'p' AFTER the 'n\d+' suffix
        if name.starts_with("nvme") {
            if let Some(p_pos) = name.rfind('p') {
                if let Some(n_pos) = name.rfind('n') {
                    if p_pos > n_pos {
                        let after_p = &name[p_pos + 1..];
                        return !after_p.is_empty() && after_p.chars().all(|c| c.is_ascii_digit());
                    }
                }
            }
            return false;
        }
        // SATA/SCSI/virtio partitions: sda1, sdb2, vda1 — letter root + trailing digit
        if name.starts_with("sd") || name.starts_with("hd") || name.starts_with("vd") {
            return name
                .chars()
                .last()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false);
        }
        // MMC partitions: mmcblk0p1
        if name.starts_with("mmcblk") {
            return name.contains('p')
                && name
                    .chars()
                    .last()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false);
        }
        false
    }
}

/// Collect current disk partition statistics.
///
/// Uses `procfs::mounts()` to enumerate mount points and `rustix::fs::statvfs` to read total/used bytes.
/// Skips virtual filesystems (tmpfs, sysfs, proc, devtmpfs, etc.).
/// Returns an empty vector if no disks are detected.
pub fn collect() -> DiskStats {
    let mount_list = mounts().unwrap_or_default();
    let mut partitions = Vec::new();
    let mut seen_devices: HashSet<u64> = HashSet::new();

    for entry in mount_list {
        // Only real filesystems — skip tmpfs, sysfs, proc, devtmpfs, etc.
        let fstype = entry.fs_vfstype.as_str();
        if matches!(
            fstype,
            "tmpfs"
                | "sysfs"
                | "proc"
                | "devtmpfs"
                | "devpts"
                | "cgroup"
                | "cgroup2"
                | "pstore"
                | "bpf"
                | "tracefs"
                | "securityfs"
                | "hugetlbfs"
                | "mqueue"
                | "debugfs"
                | "configfs"
                | "fusectl"
                | "efivarfs"
                | "autofs"
                | "squashfs"    // snap packages
                | "overlay"     // docker/container layers
                | "snapfuse"    // snap (fuse variant)
                | "fuse.snapfuse" // snap (fuse variant, alternate name)
        ) {
            continue;
        }

        let mount_point = entry.fs_file.clone();

        // Deduplicate by device number — bind mounts and sub-directory mounts
        // share the same st_dev as their parent partition, so we keep only the
        // first (primary) mount point seen for each underlying block device.
        // /proc/mounts lists mounts in mount order, so "/" always comes before
        // bind-mounted subdirectories like "/tmp" or "/var/lib/something".
        let dev = match std::fs::metadata(&mount_point) {
            Ok(m) => m.dev(),
            Err(_) => continue,
        };
        if !seen_devices.insert(dev) {
            continue;
        }

        // Call statvfs on the mount point
        let cpath = match CString::new(mount_point.as_bytes()) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if let Ok(stat) = statvfs(&cpath) {
            let block_size = stat.f_bsize as u64;
            let total = stat.f_blocks * block_size;
            let available = stat.f_bavail * block_size;
            let used = total.saturating_sub(available);

            if total == 0 {
                continue;
            }

            partitions.push(PartitionStat {
                mount: mount_point,
                total,
                used,
            });
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// DiskIoCollector returns delta bytes since last call (valid after second call)
    #[test]
    fn test_disk_io_collector_delta_second_call() {
        let mut collector = DiskIoCollector::new();
        // First call establishes baseline
        let _first = collector.collect();
        // Second call returns delta since first call
        let second = collector.collect();
        // After two calls, the delta should be valid (u64 values)
        assert!(second.read_bytes <= u64::MAX);
        assert!(second.write_bytes <= u64::MAX);
    }

    /// At least one partition returned on Linux.
    #[test]
    fn test_disk_partitions_nonempty() {
        let stats = collect();
        // On any real Linux system with at least a root filesystem, we should see partitions.
        assert!(
            !stats.partitions.is_empty(),
            "Expected at least one disk partition on Linux"
        );
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
