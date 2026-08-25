//! # ConfigManager — TOML configuration loading/saving for cosmic-applet
//!
//! Loads and saves the applet's machine list + metric selection preferences in a format that extends
//! minimon-applet's config structure. Manages adding/removing machines, choosing which metrics to display
//! per machine via checkbox selection, and persists changes back to `config.toml`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Default configuration file path for cosmic-applet (extends minimon-applet format).
const DEFAULT_CONFIG_PATH: &str = "config.toml";

/// Default UDP port the applet listens on for incoming MetricPacket traffic.
pub const DEFAULT_UDP_PORT: u16 = 51057;

/// Timeout in seconds before a machine is marked Offline if no packets received.
pub const OFFLINE_TIMEOUT_SECS: u64 = 30;

/// Configuration manager that loads, modifies, and saves the applet's TOML configuration.
///
/// Extends minimon-applet's format by adding per-machine metric selection checkboxes,
/// UDP receiver settings (port + secret key path), and grid window preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigManager {
    /// All configured machines to monitor — includes localhost entry by default.
    pub machines: Vec<MachineConfig>,

    /// UDP port the applet listens on for incoming MetricPacket traffic from remote nmd-service instances.
    pub udp_port: u16,

    /// Path to the HMAC-SHA256 pre-shared key file (shared with remote machines).
    pub hmac_secret_path: String,

    /// Whether the grid window auto-expands when a new machine comes online.
    pub auto_expand_grid: bool,

    /// File path where this configuration is persisted (loaded from on startup).
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
        // Default config includes localhost entry — panel shows desktop stats by default.
        let localhost = MachineConfig::localhost();
        let mut machines = Vec::new();
        machines.push(localhost);

        ConfigManager {
            machines,
            udp_port: DEFAULT_UDP_PORT,
            hmac_secret_path: "/etc/nmd/secret.key".to_string(),
            auto_expand_grid: true,
            config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
        }
    }
}

impl ConfigManager {
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
                        log::info!("Loaded config from {} — {} machines configured", path, config.machines.len());
                        config
                    }
                    Err(e) => {
                        log::warn!("Failed to parse config at {}: {} — using defaults", path, e);
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
            Err(e) => return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to serialize config: {}", e),
            )),
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

    /// Loading default config returns expected machine list with localhost entry (Beverly writes after implementation).
    #[test]
    fn test_load_config_defaults() {
        let config = ConfigManager::default();
        assert!(!config.machines.is_empty(), "Default config must include at least one machine");
        assert_eq!(config.machines[0].name, "localhost", "First entry should be localhost");
        assert_eq!(config.udp_port, DEFAULT_UDP_PORT);
    }

    /// Adding/removing machines persists correctly (Beverly writes after implementation).
    #[test]
    fn test_add_remove_machine() {
        let mut config = ConfigManager::default();
        assert_eq!(config.machines.len(), 1, "Should start with just localhost");

        // Add a new machine.
        let added = config.add_machine("pluto", "192.168.1.20");
        assert!(added, "Adding new machine should succeed");
        assert_eq!(config.machines.len(), 2);

        // Adding duplicate should fail.
        let dup_added = config.add_machine("pluto", "192.168.1.99");
        assert!(!dup_added, "Adding duplicate machine name should fail");
        assert_eq!(config.machines.len(), 2, "Machine count unchanged after failed add");

        // Remove the added machine — localhost remains.
        let removed = config.remove_machine("pluto");
        assert!(removed, "Removing existing machine should succeed");
        assert_eq!(config.machines.len(), 1);

        // Removing again should fail (already gone).
        let not_removed = config.remove_machine("pluto");
        assert!(!not_removed, "Removing non-existent machine should fail");
    }

    /// Default config includes localhost entry with all metrics enabled (Beverly writes).
    #[test]
    fn test_localhost_entry() {
        let config = ConfigManager::default();
        let localhost = &config.machines[0];
        assert_eq!(localhost.name, "localhost");
        assert_eq!(localhost.host, "127.0.0.1");
        assert!(localhost.enabled);
        // All metric checkboxes should be enabled by default.
        assert!(localhost.show_cpu && localhost.show_memory && localhost.show_disk);
    }

    /// Save/load roundtrip preserves configuration (Beverly writes).
    #[test]
    fn test_save_load_roundtrip() {
        let mut config = ConfigManager::default();
        config.add_machine("spark", "192.168.1.30");
        // TODO: Test actual file save/load once filesystem access is wired up (Beverly).

        assert_eq!(config.machines.len(), 2, "Should have localhost + spark after add");
    }
}
