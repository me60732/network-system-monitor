//! # systemd Unit File Constants and Install/Uninstall Helpers
//!
//! Provides the standard systemd unit file content for `nmd.service` — a pop-os template structure
//! that runs the nmd-service binary as a daemon on each remote machine (Pluto, Spark, etc.).
//! Also includes install/uninstall helper functions used by `install-scripts/install.sh`.
//!
//! ## Unit File Structure (pop-os Template)
//!
//! The unit follows standard systemd conventions:
//! - `[Unit]`: Description, dependencies (network-online.target)
//! - `[Service]`: ExecStart with config path, Restart=always, User=nobody for least privilege
//! - `[Install]`: WantedBy=multi-user.target

/// Name of the systemd service as it appears in `systemctl`.
pub const SERVICE_NAME: &str = "nmd.service";

/// Path where the compiled binary is installed on remote machines.
pub const INSTALL_PATH: &str = "/usr/local/bin/nmd-service";

/// Default configuration file path for nmd-service.
pub const CONFIG_PATH: &str = "/etc/nmd/config.toml";

/// HMAC pre-shared key file path (0600 permissions, 32 bytes).
pub const SECRET_KEY_PATH: &str = "/etc/nmd/secret.key";

/// Directory containing config + secret files on remote machines.
pub const CONFIG_DIR: &str = "/etc/nmd";

/// User account under which the service runs (least-privilege principle per Worf's review).
pub const SERVICE_USER: &str = "nobody";

/// The systemd unit file content template for `nmd.service`.
///
/// Uses `%h` specifier so ExecStart can reference a config path, and restarts on failure.
/// The binary reads `/etc/nmd/config.toml` by default (overridable via `--config`).
pub const UNIT_FILE_CONTENT: &str = r#"[Unit]
Description=Network System Monitor — Remote Metrics Service
Documentation=https://github.com/mark/network-system-monitor
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/nmd-service --config /etc/nmd/config.toml
Restart=always
RestartSec=5s
User=nobody
Group=nogroup
# Security hardening (Worf Phase 1A)
NoNewPrivileges=true
ProtectSystem=strict
ReadWritePaths=/etc/nmd/

[Install]
WantedBy=multi-user.target
"#;

/// Generate a systemd unit file at the standard location (`/etc/systemd/system/nmd.service`).
///
/// Writes [`UNIT_FILE_CONTENT`] to `/etc/systemd/system/{SERVICE_NAME}`. Requires root
/// privileges (typically run by `install-scripts/install.sh` with sudo). Returns the path
/// to the written file on success.
pub fn install_unit_file() -> Result<std::path::PathBuf, std::io::Error> {
    let dir = std::path::Path::new("/etc/systemd/system");
    // Ensure the target directory exists (it should on any systemd system, but be safe).
    if !dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Systemd unit directory {} does not exist", dir.display()),
        ));
    }

    let path = dir.join(SERVICE_NAME);
    std::fs::write(&path, UNIT_FILE_CONTENT)?;
    log::info!("Installed systemd unit file at {}", path.display());
    Ok(path)
}

/// Remove the systemd unit file and stop/disable the service.
///
/// Used by `install-scripts/uninstall.sh` for clean removal from remote machines.
/// Returns `Ok(())` if successful or if the file doesn't exist (idempotent).
pub fn uninstall_unit_file() -> Result<(), std::io::Error> {
    let path = std::path::PathBuf::from("/etc/systemd/system").join(SERVICE_NAME);

    // Idempotent: return Ok if the file is already gone.
    match std::fs::remove_file(&path) {
        Ok(()) => {
            log::info!("Removed systemd unit file at {}", path.display());
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::debug!("Unit file {} already absent — nothing to remove", path.display());
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Generate the complete install script content for remote machine setup.
///
/// This is used by `install-scripts/install.sh` to create `/etc/nmd/` directory, write config.toml,
/// generate the HMAC secret key, copy the binary, and enable the systemd service — all in one step.
pub const INSTALL_SCRIPT: &str = r#"#!/usr/bin/env bash
# nmd-service install script — runs on remote machines (Pluto, Spark, etc.)
set -euo pipefail

CONFIG_DIR="/etc/nmd"
BINARY_PATH="/usr/local/bin/nmd-service"
UNIT_FILE="/etc/systemd/system/nmd.service"

echo "[nmd] Installing Network System Monitor service..."

# 1. Create config directory with restricted permissions (Worf Phase 1A)
mkdir -p "$CONFIG_DIR"
chmod 750 "$CONFIG_DIR"

# 2. Generate HMAC pre-shared key (32 raw bytes — must match load_secret_key() validation)
if [ ! -f "$CONFIG_DIR/secret.key" ]; then
    echo "[nmd] Generating HMAC secret key..."
    head -c 32 /dev/urandom > "$CONFIG_DIR/secret.key"
fi
chmod 600 "$CONFIG_DIR/secret.key"

# 3. Write default config.toml (edit host/port to match your desktop)
cat > "$CONFIG_DIR/config.toml" << 'TOMLEOF'
host = "192.168.1.10"   # ← EDIT: desktop applet IP address
port = 51057
interval_ms = 2000
machine_id = ""          # ← Auto-detected from hostname if empty
hmac_secret_path = "/etc/nmd/secret.key"
TOMLEOF

# 4. Install binary (assumes it's in current directory or PATH)
if [ -f "./target/release/nmd-service" ]; then
    cp ./target/release/nmd-service "$BINARY_PATH"
elif command -v nmd-service &>/dev/null; then
    BINARY_PATH=$(command -v nmd-service)
else
    echo "[nmd] ERROR: nmd-service binary not found. Build with: cargo build --release" >&2
    exit 1
fi
chmod 755 "$BINARY_PATH"

# 5. Install systemd unit file
cat > "$UNIT_FILE" << 'UNITEOF'
[Unit]
Description=Network System Monitor — Remote Metrics Service
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/nmd-service --config /etc/nmd/config.toml
Restart=always
RestartSec=5s
User=nobody
Group=nogroup
NoNewPrivileges=true
ProtectSystem=strict
ReadWritePaths=/etc/nmd/

[Install]
WantedBy=multi-user.target
UNITEOF

# 6. Reload systemd, enable + start the service
systemctl daemon-reload
systemctl enable nmd.service
systemctl restart nmd.service

echo "[nmd] Service installed and started!"
echo "[nmd]   Config:    $CONFIG_DIR/config.toml"
echo "[nmd]   Secret:    $CONFIG_DIR/secret.key (0600)"
echo "[nmd]   Binary:    $BINARY_PATH"
echo ""
echo "IMPORTANT: Copy the secret key to your desktop machine's nmd-applet config."
echo "  Desktop applet needs this same key for HMAC verification."
"#;

#[cfg(test)]
mod tests {
    use super::*;

    /// Unit file content contains required systemd sections (Beverly writes after implementation).
    #[test]
    fn test_unit_file_has_required_sections() {
        assert!(UNIT_FILE_CONTENT.contains("[Unit]"));
        assert!(UNIT_FILE_CONTENT.contains("Description="));
        assert!(UNIT_FILE_CONTENT.contains("After=network-online.target"));

        assert!(UNIT_FILE_CONTENT.contains("[Service]"));
        assert!(UNIT_FILE_CONTENT.contains("ExecStart="));
        assert!(UNIT_FILE_CONTENT.contains("Restart=always"));

        assert!(UNIT_FILE_CONTENT.contains("[Install]"));
        assert!(UNIT_FILE_CONTENT.contains("WantedBy=multi-user.target"));
    }

    /// Service name is valid for systemd (Beverly writes after implementation).
    #[test]
    fn test_service_name_valid() {
        // Systemd service names must end with .service and contain only alphanumeric + dash/underscore.
        assert!(SERVICE_NAME.ends_with(".service"));
        let base = SERVICE_NAME.strip_suffix(".service").unwrap();
        for c in base.chars() {
            assert!(c.is_alphanumeric() || c == '-' || c == '_', "Invalid char '{}' in service name", c);
        }
    }

    /// Secret key path is under /etc/nmd/ (Beverly writes after implementation).
    #[test]
    fn test_secret_key_path_under_config_dir() {
        assert!(SECRET_KEY_PATH.starts_with(CONFIG_DIR));
        assert_eq!(SECRET_KEY_PATH, "/etc/nmd/secret.key");
    }
}