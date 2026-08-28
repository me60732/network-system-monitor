//! # ServiceConfig — TOML-based configuration for the nmd-service systemd daemon
//!
//! Loads runtime parameters from a `config.toml` file (or falls back to built-in defaults).
//!
//! ## Configuration File Format (`config.toml`)
//!
//! ```toml
//! # Desktop applet destination address
//! host = "192.168.1.10"
//! port = 51057
//!
//! # How often to collect + send metrics (seconds)
//! refresh_interval_secs = 1
//!
//! # Unique machine identifier — auto-detected from hostname if not set
//! machine_id = "pluto"
//!
//! # Optional: receiver's X25519 public key (hex-encoded, 32 bytes)
//! # When absent, sender operates in bootstrap mode using TEMP_SHARED_KEY.
//! # After pairing is accepted, this field is populated with the real receiver pubkey.
//! receiver_pubkey = "hex-encoded-32-byte-x25519-pubkey"
//! ```
//!
//! ## Security (Pairing System V1, Phase 1)
//!
//! ChaCha20-Poly1305 AEAD encryption provides both confidentiality and authenticity.
//! The Ed25519 identity keypair is auto-generated at `~/.config/nmd/keypair.key` on first start.
//! Phase 2 will use ECDH to derive per-machine shared keys during pairing.

use serde::Deserialize;
use std::fs;

/// Default UDP destination: desktop applet listening address.
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 51057;
/// Default collection/send interval in seconds (every 1 second for accurate CPU delta measurement).
const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 1;

/// Default configuration file path for nmd-service (loaded when no --config is given).
pub const DEFAULT_CONFIG_PATH: &str = "/etc/nmd/config.toml";

/// Runtime configuration for `nmd-service`, loaded from TOML at startup.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConfig {
    /// Desktop applet IP address to send UDP packets to (default: "127.0.0.1").
    #[serde(default = "default_host")]
    pub host: String,
    /// UDP port on the desktop applet (default: 51057).
    #[serde(default = "default_port")]
    pub port: u16,
    /// Collection + transmission interval in seconds (default: 1 = every 1s for accurate CPU delta measurement).
    #[serde(default = "default_refresh_interval_secs")]
    pub refresh_interval_secs: u64,
    /// Unique machine identifier — used for auto-registration.
    /// Falls back to the system hostname if not set in config or empty string.
    #[serde(default = "default_machine_id")]
    pub machine_id: String,
    /// Receiver's X25519 public key (hex-encoded, 32 bytes) for ECDH-derived per-machine cipher.
    /// When absent (None), sender uses TEMP_SHARED_KEY (bootstrap/unpaired mode).
    #[serde(default)]
    pub receiver_pubkey: Option<String>,
}

/// Serde default functions for partial TOML config support.
fn default_host() -> String {
    DEFAULT_HOST.to_string()
}
fn default_port() -> u16 {
    DEFAULT_PORT
}
fn default_refresh_interval_secs() -> u64 {
    DEFAULT_REFRESH_INTERVAL_SECS
}
fn default_machine_id() -> String {
    "unknown".to_string()
}

impl Default for ServiceConfig {
    fn default() -> Self {
        // Auto-detect hostname as machine_id if not explicitly configured.
        let machine_id = hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".to_string());

        ServiceConfig {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            refresh_interval_secs: DEFAULT_REFRESH_INTERVAL_SECS,
            machine_id,
            receiver_pubkey: None,
        }
    }
}

impl ServiceConfig {
    /// Load configuration from a TOML file at the given path.
    ///
    /// Falls back to [`ServiceConfig::default()`] if the file doesn't exist or is malformed,
    /// merging any provided fields over defaults so partial configs are valid. Fields not present
    /// in the TOML use `#[serde(default)]` semantics — `machine_id` falls back to auto-detected hostname.
    pub fn load(config_path: &str) -> Self {
        let mut config = ServiceConfig::default();

        match fs::read_to_string(config_path) {
            Ok(contents) => {
                // Parse TOML into a partial ServiceConfig, then merge over defaults.
                // Fields not in the TOML retain their default values via serde's #[serde(default)].
                match toml::from_str::<ServiceConfig>(&contents) {
                    Ok(parsed) => {
                        config.host = parsed.host;
                        config.port = parsed.port;
                        config.refresh_interval_secs = parsed.refresh_interval_secs;
                        // Only override machine_id if the TOML actually specified one (non-empty).
                        if !parsed.machine_id.is_empty() && parsed.machine_id != "unknown" {
                            config.machine_id = parsed.machine_id;
                        }
                        log::info!(
                            "Loaded config from {} — host={}, port={}, refresh_interval={}s",
                            config_path,
                            config.host,
                            config.port,
                            config.refresh_interval_secs
                        );
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to parse config at {}: {} — using defaults (machine_id={})",
                            config_path,
                            e,
                            config.machine_id
                        );
                    }
                }
            }
            Err(_) => {
                log::info!(
                    "No config file at {}; using defaults (machine_id={})",
                    config_path,
                    config.machine_id
                );
            }
        }

        config
    }

    /// Resolve the destination `SocketAddr` for UDP packets.
    pub fn dest_addr(&self) -> std::net::SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("Invalid host/port in ServiceConfig")
    }
}

/// Lightweight hostname detection without pulling in the `hostname` crate.
mod hostname {
    use std::process::Command;

    pub fn get() -> Result<std::ffi::OsString, std::io::Error> {
        let output = Command::new("hostname").output()?;
        if output.status.success() {
            Ok(std::str::from_utf8(&output.stdout)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                .trim()
                .to_string()
                .into())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "hostname command failed",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Default config has valid host/port/interval (Beverly writes after implementation).
    #[test]
    fn test_config_defaults() {
        let config = ServiceConfig::default();
        assert!(!config.host.is_empty());
        assert!(config.port > 0);
        assert!(config.refresh_interval_secs >= 1); // Minimum sane interval
        assert!(!config.machine_id.is_empty());
    }

    /// dest_addr parses correctly from host + port (Beverly writes after implementation).
    #[test]
    fn test_dest_addr_parses() {
        let config = ServiceConfig::default();
        let addr = config.dest_addr();
        assert_eq!(addr.port(), DEFAULT_PORT);
    }

    /// Loading non-existent config file falls back to defaults (Beverly writes after implementation).
    #[test]
    fn test_load_nonexistent_config() {
        let config = ServiceConfig::load("/nonexistent/path/config.toml");
        assert_eq!(config.port, DEFAULT_PORT);
        assert_eq!(config.refresh_interval_secs, DEFAULT_REFRESH_INTERVAL_SECS);
    }
}
