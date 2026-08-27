//! Intel GPU statistics via sysfs (DRM).
//!
//! Currently unimplemented - Intel integrated GPUs don't reliably expose
//! utilization stats via sysfs. Returns None for now.

use super::GpuStats;

/// Collect Intel GPU stats from sysfs.
///
/// Currently unimplemented. Returns None.
pub fn collect() -> Option<GpuStats> {
    // Intel integrated GPUs don't have reliable sysfs interfaces for:
    // - GPU utilization (no gpu_busy_percent equivalent)
    // - VRAM usage (shares system memory, no dedicated VRAM files)
    // - Temperature (varies by chipset, no consistent hwmon interface)
    //
    // Future: Could use intel_gpu_top or DRM ioctls, but that's beyond Phase 2 scope.
    None
}
