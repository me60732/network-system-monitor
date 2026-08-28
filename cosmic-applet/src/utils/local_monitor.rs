//! # LocalMonitor — Opens desktop's own system monitor from applet click handler
//!
//! When the user clicks a "View local stats" button in the panel or grid window, this module launches
//! the appropriate system monitoring application (cosmic-system-monitor, gnome-system-monitor, etc.).

use std::process::Command;

/// LocalMonitor handles launching the desktop's native system monitor application.
/// Attempts to find an installed monitor app and spawns it as a detached process.
pub struct LocalMonitor {
    /// Command name of the preferred system monitor binary (e.g., "cosmic-system-monitor", "gnome-system-monitor").
    pub monitor_cmd: String,

    /// Whether to use the system's default browser/system-monitor detection logic.
    pub auto_detect: bool,
}

impl LocalMonitor {
    /// Create a new LocalMonitor with automatic monitor app detection enabled.
    pub fn new() -> Self {
        let cmd = Self::detect_monitor_app();
        LocalMonitor {
            monitor_cmd: cmd,
            auto_detect: true,
        }
    }

    /// Detect which system monitor application is installed on the desktop.
    /// Checks for cosmic-system-monitor first (preferred), then gnome-system-monitor as fallback.
    fn detect_monitor_app() -> String {
        // Check common system monitor binaries in order of preference.
        let candidates = [
            "cosmic-system-monitor", // Preferred — matches Cosmic desktop environment.
            "gnome-system-monitor",  // GNOME fallback.
            "htop",                  // Terminal-based fallback (may not have GUI).
            "ksysguard",             // KDE fallback.
        ];

        for cmd in &candidates {
            if Self::is_command_available(cmd) {
                return cmd.to_string();
            }
        }

        // No monitor app found — default to cosmic-system-monitor (will show error on launch).
        log::warn!("No system monitor application detected — defaulting to cosmic-system-monitor");
        "cosmic-system-monitor".to_string()
    }

    /// Check if a command is available on the system PATH.
    fn is_command_available(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Launch the detected system monitor application as a detached process.
    /// Returns Ok(()) if launched successfully, or an error if the binary isn't found/executable.
    pub fn open(&self) -> Result<(), std::io::Error> {
        log::info!("Launching system monitor: {}", self.monitor_cmd);

        // Spawn the monitor app as a detached process (don't block the applet).
        let child = Command::new(&self.monitor_cmd).spawn()?; // TODO: Add proper error handling + fallback chain (Beverly implements after review).

        log::info!("System monitor launched with PID {}", child.id());
        Ok(())
    }

    /// Open the system monitor — entry point called by PanelWidget click handler.
    /// This is a convenience method that creates a LocalMonitor and opens it in one call.
    pub fn open_system_monitor() -> Result<(), std::io::Error> {
        let monitor = LocalMonitor::new();
        monitor.open()
    }
}

impl Default for LocalMonitor {
    fn default() -> Self {
        LocalMonitor::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Detects an installed system monitor application (Beverly writes after implementation).
    #[test]
    fn test_detect_monitor_app() {
        let cmd = LocalMonitor::detect_monitor_app();
        // Should return one of the known candidate commands.
        assert!(
            [
                "cosmic-system-monitor",
                "gnome-system-monitor",
                "htop",
                "ksysguard"
            ]
            .contains(&cmd.as_str()),
            "Detected monitor command should be a known candidate, got: {}",
            cmd
        );
    }

    /// LocalMonitor initializes with auto-detect enabled.
    #[test]
    fn test_default_init() {
        let monitor = LocalMonitor::default();
        assert!(
            monitor.auto_detect,
            "Auto-detection should be enabled by default"
        );
        // Monitor command should be set to a detected or fallback value.
        assert!(
            !monitor.monitor_cmd.is_empty(),
            "Monitor command should not be empty"
        );
    }
}
