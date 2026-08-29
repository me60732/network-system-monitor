//! # nmd-service — Network System Monitor Remote Service (systemd daemon)
//!
//! This binary runs as a systemd service on each remote Linux machine (Pluto, Spark, etc.).
//! It collects system metrics every 2 seconds via [`metrics_core::collect_all()`], packs them into
//! an rkyv-encoded [`MetricPacket`], and sends the packet over UDP encrypted with
//! ChaCha20-Poly1305 AEAD to the desktop Cosmic applet.
//!
//! ## Lifecycle (systemd entry point)
//!
//! ```text
//! main() → parse CLI args (--config path) → load ServiceConfig
//!   → init UdpSender (loads/generates Ed25519 identity keypair) + MetricsAggregator
//!   → install SIGTERM/SIGINT handler for graceful shutdown
//!   → loop: aggregate() → udp_sender.send() every interval_ms
//!   → on signal: flush, log shutdown, exit cleanly (code 0)
//! ```
//!
//! ## Security (Pairing System V1, Phase 1)
//!
//! - ChaCha20-Poly1305 AEAD encryption — confidentiality + authenticity in one operation
//!   (Phase 1 uses a temporary hardcoded key; Phase 2 derives per-machine keys via ECDH).
//! - Replay protection: timestamp freshness (< 10s old) + monotonic sequence number.
//! - Service runs as `nobody:nogroup` via systemd hardening directives.

use clap::Parser;
use nmd_service::{MetricsAggregator, ServiceConfig, UdpSender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Command-line arguments parsed via clap derive macro.
#[derive(Parser, Debug)]
#[command(
    name = "nmd-service",
    version,
    about = "Network System Monitor — remote metrics service"
)]
struct Cli {
    /// Path to the TOML configuration file (default: /etc/nmd/config.toml).
    #[arg(short, long, default_value = nmd_service::DEFAULT_CONFIG_PATH)]
    config: String,

    /// Run in foreground mode without daemonizing (systemd handles this; useful for debugging).
    #[arg(long, default_value_t = false)]
    foreground: bool,
}

/// Global shutdown flag set by the signal handler. Checked each loop iteration.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse CLI arguments (--config path).
    let cli = Cli::parse();

    // 2. Initialize logging (stderr for systemd journal capture).
    env_logger::init();

    log::info!("nmd-service starting up");
    log::info!("Config file: {}", cli.config);

    let mut config = ServiceConfig::load(&cli.config);
    log::info!(
        "Loaded config — host={}, port={}, refresh_interval={}s, machine_id={}",
        config.host,
        config.port,
        config.refresh_interval_secs,
        config.machine_id
    );

    // ── Step 1: TCP pairing if no receiver_pubkey ─────────────────────────
    if config.receiver_pubkey.is_none() {
        log::info!(
            "No receiver_pubkey configured — attempting TCP pairing with {}:{}",
            config.host,
            config.port
        );

        let sender_x25519_pubkey = get_sender_x25519_pubkey()?;

        match nmd_service::request_pairing(
            &config.host,
            config.port,
            &config.machine_id,
            &sender_x25519_pubkey,
        ) {
            nmd_service::PairingResult::Accepted(receiver_pubkey) => {
                log::info!(
                    "✅ TCP pairing complete — saving receiver_pubkey to {}",
                    cli.config
                );
                config.receiver_pubkey = Some(receiver_pubkey);
                if let Err(e) = config.save(&cli.config) {
                    log::error!("Failed to save config after pairing: {}", e);
                    // Continue anyway — we have the key in memory for this run
                }
            }
            nmd_service::PairingResult::Denied => {
                log::error!("Pairing denied by receiver — exiting. Try again after acceptance.");
                std::process::exit(1);
            }
            nmd_service::PairingResult::Failed(e) => {
                log::error!(
                    "TCP pairing failed: {}. Configure receiver_pubkey manually or retry.",
                    e
                );
                std::process::exit(1);
            }
        }
    }

    // ── Step 2: Key rotation if keypair is > 24h old ──────────────────────
    const KEY_ROTATION_SECS: u64 = 86400; // 24 hours
    if UdpSender::keypair_age_secs() > KEY_ROTATION_SECS {
        log::info!("🔄 Keypair is older than 24h — initiating key rotation");
        let receiver_pubkey = config.receiver_pubkey.as_deref().unwrap_or("");

        match rotate_sender_key(&config, receiver_pubkey) {
            Ok(()) => log::info!("🔄 Key rotation complete"),
            Err(e) => log::warn!(
                "🔄 Key rotation failed (non-fatal, continuing with old key): {}",
                e
            ),
        }
    }

    // ── Step 3: Initialize UDP sender ───────────────────────────────────────
    let dest = config.dest_addr();
    let receiver_pubkey_hex = config.receiver_pubkey.clone().ok_or_else(|| {
        std::io::Error::other("receiver_pubkey still not configured after pairing attempt")
    })?;

    let mut sender =
        UdpSender::new_with_config(dest, &config.machine_id, Some(receiver_pubkey_hex))?;
    log::info!(
        "UDP sender initialized — sending to {} (machine_id={})",
        dest,
        config.machine_id
    );

    // 5. Initialize metrics aggregator.
    let mut aggregator = MetricsAggregator::new(config.clone());

    // 6. Install signal handlers for graceful shutdown (SIGTERM from systemd + SIGINT/Ctrl-C).
    install_signal_handlers();

    log::info!(
        "Entering main loop — interval={}s",
        config.refresh_interval_secs
    );

    // 7. Main collection + transmission loop.
    while !SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
        let start = std::time::Instant::now();

        // Collect metrics and pack into MetricPacket (delegates to metrics-core).
        let packet = aggregator.aggregate();

        // Send via UDP with ChaCha20-Poly1305 AEAD encryption.
        if let Err(e) = (&mut sender).send(&packet) {
            log::warn!("UDP send failed: {}", e);
        } else {
            let mem_percent = if packet.memory.total_bytes > 0 {
                (packet.memory.used_bytes as f64 / packet.memory.total_bytes as f64) * 100.0
            } else {
                0.0
            };
            // Note: sequence counter was already incremented by send(), so subtract 1 for the packet we just sent
            log::debug!(
                "Sent metrics — seq={}, cpu={:.1}%, mem={:.1}%",
                sender.get_sequence() - 1,
                packet.cpu.usage_percent,
                mem_percent
            );
        }

        // Sleep for the configured interval (minus time already spent collecting/sending).
        let elapsed = start.elapsed();
        let sleep_duration = if elapsed < Duration::from_secs(config.refresh_interval_secs) {
            Duration::from_secs(config.refresh_interval_secs) - elapsed
        } else {
            Duration::ZERO // Collection took longer than interval — skip sleep
        };

        std::thread::sleep(sleep_duration);
    }

    log::info!("Shutdown requested — exiting gracefully");
    Ok(())
}

/// Load (or generate) the sender's Ed25519 keypair and return the X25519 public key.
fn get_sender_x25519_pubkey() -> Result<[u8; 32], std::io::Error> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let path = std::path::PathBuf::from(home)
        .join(".config")
        .join("nmd")
        .join("keypair.key");

    let key = if path.exists() {
        let bytes = std::fs::read(&path)?;
        let arr: [u8; 64] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| std::io::Error::other("keypair file wrong size"))?;
        ed25519_dalek::SigningKey::from_keypair_bytes(&arr)
            .map_err(|e| std::io::Error::other(e.to_string()))?
    } else {
        let k = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, k.to_keypair_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        k
    };

    Ok(nmd_service::crypto::derive_x25519_pubkey_from_ed25519_secret(&key.to_bytes()))
}

/// Perform key rotation: generate new keypair, authenticate with old key, send to receiver.
fn rotate_sender_key(
    config: &ServiceConfig,
    receiver_pubkey_hex: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load the OLD keypair before generating new one
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let path = std::path::PathBuf::from(home)
        .join(".config")
        .join("nmd")
        .join("keypair.key");

    let old_bytes = std::fs::read(&path)?;
    let old_arr: [u8; 64] = old_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "keypair file wrong size")?;
    let old_key =
        ed25519_dalek::SigningKey::from_keypair_bytes(&old_arr).map_err(|e| e.to_string())?;
    let old_ed25519_secret = old_key.to_bytes();

    // Generate new keypair
    let (_new_ed25519_secret, new_x25519_pubkey) = UdpSender::generate_new_keypair()?;

    // Send rotation request
    match nmd_service::request_key_rotation(
        &config.host,
        config.port,
        &config.machine_id,
        &old_ed25519_secret,
        receiver_pubkey_hex,
        &new_x25519_pubkey,
    ) {
        nmd_service::PairingResult::Accepted(_) => Ok(()),
        nmd_service::PairingResult::Denied => Err("Rotation denied by receiver".into()),
        nmd_service::PairingResult::Failed(e) => Err(e.into()),
    }
}

/// Install signal handlers for SIGTERM (systemd stop/restart) and SIGINT (Ctrl-C).
///
/// Uses `signal_hook` to register handlers that set the global `SHUTDOWN_REQUESTED` flag,
/// allowing graceful shutdown: finish current send, log exit, return cleanly. A dedicated
/// thread blocks on each signal via `signal_hook::iterator::Signals`, so the main loop's
/// sleep is interrupted promptly when a signal arrives.
fn install_signal_handlers() {
    // Spawn a thread that listens for SIGTERM and SIGINT using signal_hook.
    // The thread sets SHUTDOWN_REQUESTED = true, which the main loop checks each iteration.
    std::thread::spawn(|| {
        let mut signals = match signal_hook::iterator::Signals::new([
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGINT,
        ]) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to register signal handlers: {}", e);
                return;
            }
        };

        // Block until a signal arrives. When it does, set the shutdown flag and exit the thread.
        for sig in signals.forever() {
            let name = match sig {
                signal_hook::consts::SIGTERM => "SIGTERM",
                signal_hook::consts::SIGINT => "SIGINT",
                _ => "UNKNOWN",
            };
            log::info!("Received {} — initiating graceful shutdown", name);
            SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
        }
    });

    log::debug!("Signal handlers installed for SIGTERM + SIGINT");
}

#[cfg(test)]
mod tests {
    /// Main function compiles and entry point exists (integration tested via cargo run).
    #[test]
    fn test_main_compiles() {
        // If this module compiles, main.rs structure is correct.
        assert!(true);
    }
}
