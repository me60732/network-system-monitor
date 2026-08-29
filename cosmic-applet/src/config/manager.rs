//! # ConfigManager — TOML configuration loading/saving for cosmic-applet
//!
//! Loads and saves the applet's machine list + metric selection preferences in a format that extends
//! minimon-applet's config structure. Manages adding/removing machines, choosing which metrics to display
//! per machine via checkbox selection, and persists changes back to `~/.config/cosmic-applet/config.toml`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Returns the canonical config path: ~/.config/cosmic-applet/config.toml
pub fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("cosmic-applet")
        .join("config.toml")
}

/// Default UDP port the applet listens on for incoming MetricPacket traffic.
pub const DEFAULT_UDP_PORT: u16 = 51057;

/// Timeout in seconds before a machine is marked Offline if no packets received.
pub const OFFLINE_TIMEOUT_SECS: u64 = 30;

/// Configuration manager that loads, modifies, and saves the applet's TOML configuration.
///
/// Extends minimon-applet's format by adding per-machine metric selection checkboxes,
/// UDP receiver settings (port), and grid window preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigManager {
    /// All configured machines to monitor — includes localhost entry by default.
    pub machines: Vec<MachineConfig>,

    /// UDP port the applet listens on for incoming MetricPacket traffic from remote nmd-service instances.
    pub udp_port: u16,

    /// Whether the grid window auto-expands when a new machine comes online.
    #[serde(default = "default_true")]
    pub auto_expand_grid: bool,

    /// File path where this configuration is persisted (loaded from on startup).
    #[serde(skip)]
    pub config_path: PathBuf,
}

/// Configuration for a single monitored machine in the applet's config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConfig {
    /// Human-readable name — matches nmd-service's machine_id field in MetricPacket.
    pub name: String,

    /// Whether this machine is enabled (shown in grid + receiving UDP data).
    pub enabled: bool,

    /// IP address or hostname of the remote machine running nmd-service.
    pub host: String,

    /// Port number for the remote nmd-service UDP sender (default: 51057).
    pub port: u16,

    // ── Per-Machine Metric Selection (checkbox selection — Troi completes UI) ───────────
    /// Whether to display CPU usage percentage in grid panel.
    #[serde(default = "default_true")]
    pub show_cpu: bool,

    /// Whether to display memory usage percentage in grid panel.
    #[serde(default = "default_true")]
    pub show_memory: bool,

    /// Whether to display disk usage percentage in grid panel.
    #[serde(default = "default_true")]
    pub show_disk: bool,

    /// Whether to display network RX/TX rate in grid panel.
    #[serde(default = "default_true")]
    pub show_network: bool,

    /// Whether to display uptime in grid panel.
    #[serde(default = "default_true")]
    pub show_uptime: bool,

    /// Whether to display GPU VRAM usage (if available) in grid panel.
    #[serde(default = "default_true")]
    pub show_gpu_vram: bool,

    /// Whether to display temperature (°C) in grid panel.
    #[serde(default = "default_true")]
    pub show_temperature: bool,
}

/// Default value for boolean serde fields — returns true so metrics are shown by default.
fn default_true() -> bool {
    true
}

impl MachineConfig {
    /// Create a new machine configuration with the given name and host, all metrics enabled by default.
    pub fn new(name: String, host: String) -> Self {
        MachineConfig {
            name,
            enabled: true,
            host,
            port: DEFAULT_UDP_PORT,
            show_cpu: true,
            show_memory: true,
            show_disk: true,
            show_network: true,
            show_uptime: true,
            show_gpu_vram: true,
            show_temperature: true,
        }
    }

    /// Create the default localhost entry — represents the desktop machine's own stats.
    pub fn localhost() -> Self {
        MachineConfig::new("localhost".to_string(), "127.0.0.1".to_string())
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        // Default config has empty machine list — machines are added through pairing.
        ConfigManager {
            machines: Vec::new(),
            udp_port: DEFAULT_UDP_PORT,
            auto_expand_grid: true,
            config_path: default_config_path(),
        }
    }
}

impl ConfigManager {
    /// Validate the configuration and return detailed error messages if invalid.
    /// Returns Ok(()) if valid, Err with human-readable error message otherwise.
    pub fn validate(&self) -> Result<(), String> {
        // Empty machine list is now valid (machines are added through pairing)

        // Check for duplicate machine names
        let mut seen_names = std::collections::HashSet::new();
        for machine in &self.machines {
            if !seen_names.insert(&machine.name) {
                return Err(format!(
                    "Duplicate machine name '{}' found — each machine must have a unique name",
                    machine.name
                ));
            }
        }

        // Validate UDP port range
        if self.udp_port == 0 {
            return Err(
                "UDP port cannot be 0 — choose a port between 1024-65535 (recommend 51057)"
                    .to_string(),
            );
        }

        // Validate machine configurations
        for machine in &self.machines {
            if machine.name.trim().is_empty() {
                return Err("Machine name cannot be empty".to_string());
            }

            if machine.host.trim().is_empty() {
                return Err(format!(
                    "Machine '{}' has empty host — specify IP address or hostname",
                    machine.name
                ));
            }

            if machine.port == 0 {
                return Err(format!("Machine '{}' has invalid port 0", machine.name));
            }
        }

        Ok(())
    }

    /// Load configuration from a TOML file at the given path.
    /// Falls back to default config (with localhost entry) if file doesn't exist or is malformed.
    pub fn load(path: &str) -> Self {
        let config_path = PathBuf::from(path);

        // Attempt to read and parse the TOML config file.
        match std::fs::read_to_string(&config_path) {
            Ok(content) => {
                match toml::from_str::<ConfigManager>(&content) {
                    Ok(mut config) => {
                        config.config_path = config_path;

                        // Validate loaded config
                        if let Err(validation_error) = config.validate() {
                            log::error!(
                                "Config validation failed: {} — using defaults",
                                validation_error
                            );
                            log::error!("   Using default configuration instead.");
                            log::error!("   Fix {} and restart the applet.", path);
                            let mut default = ConfigManager::default();
                            default.config_path = config.config_path;
                            return default;
                        }

                        log::info!(
                            "Loaded config from {} — {} machines configured",
                            path,
                            config.machines.len()
                        );
                        config
                    }
                    Err(e) => {
                        log::error!("Failed to parse TOML at {}: {} — using defaults", path, e);
                        log::error!("   Check TOML syntax in {}", path);
                        log::error!("   Using default configuration instead.");
                        let mut default = ConfigManager::default();
                        default.config_path = config_path;
                        default
                    }
                }
            }
            Err(_) => {
                // File doesn't exist or can't be read — use built-in defaults (includes localhost).
                log::info!("Config file {} not found — using defaults", path);
                let mut config = ConfigManager::default();
                config.config_path = config_path;
                config
            }
        }
    }

    /// Save the current configuration to a TOML file.
    /// Creates parent directories if they don't exist.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let content = match toml::to_string(self) {
            Ok(c) => c,
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to serialize config: {}", e),
                ));
            }
        };

        // Create parent directory if it doesn't exist.
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&self.config_path, content)?;
        log::info!("Saved config to {}", self.config_path.display());
        Ok(())
    }

    /// Add a new machine to the configuration. Returns false if a machine with that name already exists.
    pub fn add_machine(&mut self, name: &str, host: &str) -> bool {
        // Check for duplicate names — each machine_id must be unique per Worf's security analysis.
        if self.machines.iter().any(|m| m.name == name) {
            log::warn!("Machine '{}' already exists in config", name);
            return false;
        }

        let machine = MachineConfig::new(name.to_string(), host.to_string());
        self.machines.push(machine);
        true
    }

    /// Remove a machine from the configuration by name. Returns true if removed, false if not found.
    pub fn remove_machine(&mut self, name: &str) -> bool {
        let initial_len = self.machines.len();
        self.machines.retain(|m| m.name != name);

        if self.machines.len() < initial_len {
            log::info!("Removed machine '{}' from config", name);
            true
        } else {
            false
        }
    }

    /// Find a machine by name — returns reference or None.
    pub fn find_machine(&self, name: &str) -> Option<&MachineConfig> {
        self.machines.iter().find(|m| m.name == name)
    }

    /// Return the list of enabled machines for UDP receiver registration.
    pub fn enabled_machines(&self) -> Vec<&MachineConfig> {
        self.machines.iter().filter(|m| m.enabled).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loading default config returns expected empty machine list (Beverly writes after implementation).
    #[test]
    fn test_load_config_defaults() {
        let config = ConfigManager::default();
        assert!(
            config.machines.is_empty(),
            "Default config should have no machines - they are added through pairing"
        );
        assert_eq!(config.udp_port, DEFAULT_UDP_PORT);
    }

    /// Adding/removing machines persists correctly (Beverly writes after implementation).
    #[test]
    fn test_add_remove_machine() {
        let mut config = ConfigManager::default();
        assert!(config.machines.is_empty(), "Should start with no machines");

        // Add a new machine.
        let added = config.add_machine("pluto", "192.168.1.20");
        assert!(added, "Adding new machine should succeed");
        assert_eq!(config.machines.len(), 1);

        // Adding duplicate should fail.
        let dup_added = config.add_machine("pluto", "192.168.1.99");
        assert!(!dup_added, "Adding duplicate machine name should fail");
        assert_eq!(
            config.machines.len(),
            1,
            "Machine count unchanged after failed add"
        );

        // Remove the added machine.
        let removed = config.remove_machine("pluto");
        assert!(removed, "Removing existing machine should succeed");
        assert!(
            config.machines.is_empty(),
            "After removal, no machines remain"
        );

        // Removing again should fail (already gone).
        let not_removed = config.remove_machine("pluto");
        assert!(!not_removed, "Removing non-existent machine should fail");
    }

    /// MachineConfig::localhost() creates a machine for localhost (Beverly writes).
    #[test]
    fn test_localhost_entry() {
        let m = MachineConfig::localhost();
        assert_eq!(m.name, "localhost");
        assert_eq!(m.host, "127.0.0.1");
        assert!(m.enabled);
    }

    /// Save/load roundtrip preserves configuration (Beverly writes).
    #[test]
    fn test_save_load_roundtrip() {
        let temp_dir = std::env::temp_dir();
        let test_config_path = temp_dir.join(format!("test_config_{}.toml", std::process::id()));

        // Create config with custom values
        let mut config = ConfigManager::default();
        config.config_path = test_config_path.clone();
        config.add_machine("spark", "192.168.1.30");
        config.add_machine("pluto", "192.168.1.99");
        config.udp_port = 51058; // Non-default port
        config.auto_expand_grid = false;

        // Disable GPU metric on pluto machine
        if let Some(pluto) = config.machines.iter_mut().find(|m| m.name == "pluto") {
            pluto.show_gpu_vram = false;
        }

        // Save to temp file
        config.save().expect("Save should succeed");

        // Load back from file
        let loaded = ConfigManager::load(test_config_path.to_str().unwrap());

        // Verify all fields preserved
        assert_eq!(
            loaded.machines.len(),
            2,
            "Should have spark + pluto (no automatic localhost)"
        );
        assert_eq!(loaded.udp_port, 51058, "Custom UDP port preserved");
        assert_eq!(
            loaded.auto_expand_grid, false,
            "Grid expand preference preserved"
        );

        // Verify machine order and properties
        assert_eq!(loaded.machines[0].name, "spark");
        assert_eq!(loaded.machines[0].host, "192.168.1.30");
        assert_eq!(loaded.machines[1].name, "pluto");

        // Verify per-machine metric toggles preserved
        let pluto = &loaded.machines[1];
        assert!(
            !pluto.show_gpu_vram,
            "GPU toggle should be disabled for pluto"
        );
        assert!(pluto.show_cpu, "Other metrics should remain enabled");

        // Cleanup
        let _ = std::fs::remove_file(test_config_path);
    }

    /// Config validation catches common errors with helpful messages (Beverly writes).
    #[test]
    fn test_config_validation() {
        // Valid config should pass
        let valid_config = ConfigManager::default();
        assert!(
            valid_config.validate().is_ok(),
            "Default config should be valid"
        );

        // Empty machines list is valid (no validation error)
        let mut empty_machines = ConfigManager::default();
        empty_machines.machines.clear();
        assert!(
            empty_machines.validate().is_ok(),
            "Empty machine list should be valid"
        );

        // Duplicate machine names should fail
        let mut duplicate_names = ConfigManager::default();
        duplicate_names.add_machine("spark", "192.168.1.30");
        duplicate_names.machines.push(MachineConfig::new(
            "spark".to_string(),
            "192.168.1.99".to_string(),
        ));
        assert!(
            duplicate_names.validate().is_err(),
            "Duplicate names should fail validation"
        );
        let err = duplicate_names.validate().unwrap_err();
        assert!(
            err.contains("Duplicate") && err.contains("spark"),
            "Error should mention duplicate name"
        );

        // Zero UDP port should fail
        let mut zero_port = ConfigManager::default();
        zero_port.udp_port = 0;
        assert!(
            zero_port.validate().is_err(),
            "Zero UDP port should fail validation"
        );

        // Empty machine list is valid (no validation error)
        let mut empty_machines = ConfigManager::default();
        empty_machines.machines.clear();
        assert!(
            empty_machines.validate().is_ok(),
            "Empty machine list should be valid"
        );

        // Machine with empty name should fail
        let mut empty_name = ConfigManager::default();
        empty_name.add_machine("test", "127.0.0.1");
        empty_name.machines[0].name = "".to_string();
        assert!(
            empty_name.validate().is_err(),
            "Empty machine name should fail validation"
        );

        // Machine with empty host should fail
        let mut empty_host = ConfigManager::default();
        empty_host.add_machine("test", "127.0.0.1");
        empty_host.machines[0].host = "".to_string();
        assert!(
            empty_host.validate().is_err(),
            "Empty machine host should fail validation"
        );
    }
}
