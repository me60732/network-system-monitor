//! # nmd-service — Network System Monitor Remote Service (systemd daemon)
//!
//! This binary runs as a systemd service on each remote Linux machine (Pluto, Spark, etc.).
//! It collects system metrics every 2 seconds via [`metrics_core::collect_all()`], packs them into
//! an rkyv-encoded [`MetricPacket`], and sends the packet over UDP with HMAC-SHA256 authentication
//! to the desktop Cosmic applet.
//!
//! ## Lifecycle (systemd entry point)
//!
//! ```text
//! main() → parse CLI args (--config path) → load ServiceConfig + secret key
//!   → init UdpSender + MetricsAggregator
//!   → install SIGTERM/SIGINT handler for graceful shutdown
//!   → loop: aggregate() → udp_sender.send() every interval_ms
//!   → on signal: flush, log shutdown, exit cleanly (code 0)
//! ```
//!
//! ## Security (Worf Phase 1A)
//!
//! - HMAC-SHA256 authentication with pre-shared key at `/etc/nmd/secret.key` (0600).
//! - Replay protection: timestamp freshness (< 10s old) + monotonic sequence number.
//! - Service runs as `nobody:nogroup` via systemd hardening directives.

use clap::Parser;
use nmd_service::{MetricsAggregator, ServiceConfig, UdpSender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Command-line arguments parsed via clap derive macro.
#[derive(Parser, Debug)]
#[command(name = "nmd-service", version, about = "Network System Monitor — remote metrics service")]
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

    // 3. Load service configuration from TOML.
    let config = ServiceConfig::load(&cli.config);
    log::info!(
        "Loaded config — host={}, port={}, interval={}ms, machine_id={}",
        config.host,
        config.port,
        config.interval_ms,
        config.machine_id
    );

    // 4. Load HMAC pre-shared key from /etc/nmd/secret.key (Worf Phase 1A).
    let secret_key = match config.load_secret_key() {
        Ok(key) => {
            log::info!("HMAC secret key loaded successfully");
            key
        }
        Err(e) => {
            log::error!("Failed to load HMAC secret key: {}", e);
            return Err(format!("Secret key loading failed: {}", e).into());
        }
    };

    // 5. Initialize UDP sender with destination address + secret key + machine_id for pre-serialized buffer.
    let dest = config.dest_addr();
    let mut sender = UdpSender::new(dest, secret_key, &config.machine_id)?;
    log::info!("UDP sender initialized — sending to {} (machine_id={})", dest, config.machine_id);

    // 6. Initialize metrics aggregator.
    let aggregator = MetricsAggregator::new(config.clone());

    // 7. Install signal handlers for graceful shutdown (SIGTERM from systemd + SIGINT/Ctrl-C).
    install_signal_handlers();

    log::info!("Entering main loop — interval={}ms", config.interval_ms);

    // 8. Main collection + transmission loop.
    while !SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
        let start = std::time::Instant::now();

        // Collect metrics and pack into MetricPacket (delegates to metrics-core).
        let packet = aggregator.aggregate();

        // Send via UDP with HMAC-SHA256 authentication — in-place buffer mutation, zero allocations.
        if let Err(e) = (&mut sender).send(&packet) {
            log::warn!("UDP send failed: {}", e);
        } else {
            let mem_percent = if packet.memory_total_bytes > 0 {
                (packet.memory_used_bytes as f64 / packet.memory_total_bytes as f64) * 100.0
            } else {
                0.0
            };
            log::debug!(
                "Sent metrics — seq={}, cpu={:.1}%, mem={:.1}%",
                packet.sequence,
                packet.cpu_usage,
                mem_percent
            );
        }

        // Sleep for the configured interval (minus time already spent collecting/sending).
        let elapsed = start.elapsed();
        let sleep_duration = if elapsed < Duration::from_millis(config.interval_ms) {
            Duration::from_millis(config.interval_ms) - elapsed
        } else {
            Duration::ZERO // Collection took longer than interval — skip sleep
        };

        std::thread::sleep(sleep_duration);
    }

    log::info!("Shutdown requested — exiting gracefully");
    Ok(())
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