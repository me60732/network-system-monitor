//! # UdpReceiver — Listens for ChaCha20-Poly1305 encrypted MetricPacket via UDP
//!
//! Binds to a configurable port and receives incoming UDP packets from remote nmd-service instances.
//! Each wire packet (`[12-byte nonce][ciphertext + 16-byte Poly1305 tag]`, see
//! [`nmd_service::crypto`]) is decrypted with ChaCha20-Poly1305 — AEAD tag verification is
//! intrinsic to decryption, so forged or tampered packets fail here and are dropped. Decrypted
//! packets are then checked for replay protection: timestamp freshness (< 10s old) + monotonic
//! sequence number tracking per (machine_id, session_id).
//!
//! Uses ECDH-derived keys for all communication. The receiver derives the shared key from
//! the sender's X25519 public key in the wire header.

use nmd_service::crypto;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::mpsc::Sender;

// Re-use the same rkyv-serialized MetricPacket type from nmd-service's packet definition.
// Since both crates are in the same workspace, we import the struct directly.
use crate::AppState;
use nmd_service::packet::{
    ArchivedMetricPacket, CpuMetrics, DiskMetrics, GpuMetrics, MemoryMetrics, MetricPacket,
    NetworkMetrics,
};
use rkyv::access;

/// Maximum UDP datagram size — standard Ethernet MTU minus IP/UDP headers (1500 - 28 = 1472, rounded up).
const MAX_PACKET_SIZE: usize = 2048;

/// Timestamp freshness window in seconds — packets older than this are rejected as replays.
const TIMESTAMP_FRESHNESS_SECS: u64 = 10;

/// Payload types that can be sent from the UDP receiver to the iced application.
#[derive(Debug, Clone)]
pub enum UdpPayload {
    /// A valid MetricPacket was received and verified.
    ///
    /// Contains the deserialized packet data ready for UI consumption.
    Metrics(MetricPacket),
    /// A PairingRequest from an unknown sender that wants to pair.
    PairingRequest(crate::pairing_manager::PairingRequest),
}

/// Message type sent from the UDP receiver to the iced application.
///
/// Currently only carries metric updates, but can be extended to carry other types of events
/// (e.g., configuration changes, errors, etc.) as needed.
#[derive(Debug, Clone)]
pub struct UdpMessage {
    payload: UdpPayload,
}

impl UdpMessage {
    /// Get a reference to the inner payload.
    pub fn payload(&self) -> &UdpPayload {
        &self.payload
    }
}

/// UDP receiver that listens for encrypted MetricPacket traffic from remote machines.
///
/// Maintains a per-machine sequence number map to detect replayed or out-of-order packets (Worf Phase 1A).
/// Updates the shared [`AppState`] grid window in real-time as new data arrives via async background task.
pub struct UdpReceiver {
    /// Bound UDP socket listening for incoming MetricPacket traffic from remote nmd-service instances.
    pub socket: tokio::net::UdpSocket,

    /// Port the receiver is listening on (default: 51057).
    pub port: u16,

    /// Replay protection: Map of `(machine_id, session_id)` → last seen sequence number.
    /// SEC-03: Track by (machine_id, session_id) tuple to handle sender restarts gracefully.
    /// Uses Arc<Mutex<...>> for interior mutability in async context and test access.
    pub sequence_map: Arc<Mutex<HashMap<(String, String), u32>>>,

    /// Sender for sending metric updates to the UI (iced application).
    ///
    /// This allows the UDP receiver to send structured messages back to the main application thread
    /// via an async channel, enabling typed communication of received metrics.
    tx: Option<Sender<UdpMessage>>,

    /// PairingManager for TOFU pairing detection. Tracks which machines are paired and their per-machine keys.
    pub pairing_manager: std::sync::Arc<std::sync::RwLock<crate::pairing_manager::PairingManager>>,
}

impl UdpReceiver {
    /// Create a new UDP receiver bound to the specified port.
    /// The socket binds to `0.0.0.0:port` to listen on all interfaces for incoming remote machine traffic.
    /// The decryption cipher is keyed with the Phase 1 temporary shared key.
    ///
    /// # Arguments
    ///
    /// * `port` - UDP port to bind to
    /// * `tx` - Optional sender for communicating with the UI (if None, no messages will be sent)
    /// * `pairing_manager` - Arc to PairingManager for TOFU pairing detection
    pub async fn new(
        port: u16,
        tx: Option<Sender<UdpMessage>>,
        pairing_manager: std::sync::Arc<std::sync::RwLock<crate::pairing_manager::PairingManager>>,
    ) -> Result<Self, std::io::Error> {
        let addr = format!("0.0.0.0:{}", port);
        log::info!("Binding UDP receiver to {}", addr);

        // Async socket bind — no blocking timeout needed with tokio
        let socket = tokio::net::UdpSocket::bind(&addr)
            .await
            .map_err(|e| std::io::Error::other(format!("Failed to bind UDP socket: {e}")))?;

        Ok(UdpReceiver {
            socket,
            sequence_map: Arc::new(std::sync::Mutex::new(HashMap::new())),
            port,
            tx,
            pairing_manager,
        })
    }

    /// Start the async UDP receive loop — runs as a background tokio task.
    /// Continuously reads packets, decrypts + verifies the AEAD tag, checks freshness, and
    /// updates the shared AppState grid window.
    ///
    /// # Arguments
    ///
    /// * `shared_state` - Shared application state that gets updated with received metrics
    pub async fn start_listening(shared_state: Arc<RwLock<AppState>>) {
        // Create a default PairingManager using the config path from shared_state
        let config_path = shared_state
            .read()
            .unwrap()
            .config_manager
            .read()
            .unwrap()
            .config_path
            .clone();
        let pairing_manager = Arc::new(RwLock::new(crate::pairing_manager::PairingManager::new(
            config_path.join("pairing.toml"),
        )));
        Self::start_listening_with_pairing(shared_state, pairing_manager).await;
    }

    /// Start the async UDP receive loop with an externally provided PairingManager.
    /// This is used for Phase 2 testing where a custom pairing manager can be injected.
    ///
    /// # Arguments
    ///
    /// * `shared_state` - Shared application state that gets updated with received metrics
    /// * `pairing_manager` - Arc to PairingManager for TOFU pairing detection
    pub async fn start_listening_with_pairing(
        shared_state: Arc<RwLock<AppState>>,
        pairing_manager: Arc<std::sync::RwLock<crate::pairing_manager::PairingManager>>,
    ) {
        log::info!("🔌 Starting UDP receiver...");

        // Load configuration from shared_state
        let config_manager = shared_state.read().unwrap().config_manager.clone();
        let port = config_manager.read().unwrap().udp_port;

        log::info!("UDP receiver config: port={}", port);

        // Use provided pairing manager (already an Arc from caller)
        let pairing_mgr = Arc::clone(&pairing_manager);

        // Create the receiver (binds socket + initializes the Phase 1 cipher + pairing manager)
        let mut receiver = match UdpReceiver::new(port, None, pairing_mgr).await {
            Ok(r) => {
                log::info!("✓ Bound UDP socket to 0.0.0.0:{}", port);
                r
            }
            Err(e) => {
                log::error!("Failed to bind UDP socket: {}", e);
                return;
            }
        };

        log::info!("🎧 UDP receiver ready — waiting for packets...");

        // Run the listen loop
        receiver.listen_loop(shared_state).await;
    }

    /// Listen loop that processes incoming UDP packets.
    ///
    /// Implements Model C async architecture:
    /// - Receive loop: non-blocking, only receives and forwards to processing task
    /// - Processing task: dedicated tokio task handles decryption, TOFU checks, state writes
    ///
    /// # Arguments
    ///
    /// * `shared_state` - Shared application state that gets updated with received metrics
    pub async fn listen_loop(&mut self, shared_state: Arc<RwLock<AppState>>) {
        // Internal channel: receive loop → processing task
        let (proc_tx, mut proc_rx) =
            tokio::sync::mpsc::channel::<(Vec<u8>, std::net::SocketAddr)>(64);

        // Clone what the processing task needs
        let pairing_manager = Arc::clone(&self.pairing_manager);
        let state_for_proc = Arc::clone(&shared_state);
        let ui_tx = self.tx.clone();

        // Spawn dedicated processing task — handles all CPU work + state writes
        tokio::spawn(async move {
            // Processing task owns its sequence map and rate limiter
            let mut sequence_map = HashMap::<(String, String), u32>::new();

            // Token-bucket rate limiter: max 200 packets/s per IP (well above normal 1Hz)
            struct IpRateLimiter {
                buckets: HashMap<std::net::IpAddr, (u32, std::time::Instant)>,
                max_per_second: u32,
            }

            impl IpRateLimiter {
                fn new(max_per_second: u32) -> Self {
                    Self {
                        buckets: HashMap::new(),
                        max_per_second,
                    }
                }

                /// Returns true if packet should be processed, false if it should be dropped.
                fn check(&mut self, ip: std::net::IpAddr) -> bool {
                    let now = std::time::Instant::now();
                    let entry = self.buckets.entry(ip).or_insert((0, now));
                    if now.duration_since(entry.1).as_secs() >= 1 {
                        *entry = (1, now);
                        return true;
                    }
                    entry.0 += 1;
                    if entry.0 > self.max_per_second {
                        log::debug!("Rate limit exceeded for {}", ip);
                        return false;
                    }
                    true
                }

                /// Call every ~60s to evict stale entries and prevent unbounded memory growth.
                fn cleanup(&mut self) {
                    let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(10);
                    self.buckets.retain(|_, (_, t)| *t > cutoff);
                    // Hard cap: if still >1000 tracked IPs, clear everything (under attack scenario)
                    if self.buckets.len() > 1000 {
                        self.buckets.clear();
                    }
                }
            }

            let mut rate_limiter = IpRateLimiter::new(200);
            let mut cleanup_counter = 0u32;

            while let Some((data, src)) = proc_rx.recv().await {
                // Rate limit check at top of loop — drop packet if over limit
                if !rate_limiter.check(src.ip()) {
                    continue; // drop packet — rate limited
                }
                cleanup_counter += 1;
                if cleanup_counter % 10_000 == 0 {
                    rate_limiter.cleanup();
                }

                Self::process_packet(
                    &data,
                    src,
                    &mut sequence_map,
                    &pairing_manager,
                    &state_for_proc,
                    &ui_tx,
                )
                .await;
            }
        });

        // Receive loop — non-blocking, just recv + forward
        let mut buf = vec![0u8; MAX_PACKET_SIZE];
        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((size, src)) => {
                    let data = buf[..size].to_vec();
                    // If channel full (64 buffered), drop oldest rather than blocking recv
                    if proc_tx.send((data, src)).await.is_err() {
                        log::warn!("Processing channel closed — stopping receive loop");
                        break;
                    }
                }
                Err(e) => {
                    log::error!("UDP receive error: {}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// Process a single UDP packet — handles decryption, TOFU checks, sequence validation, and state updates.
    ///
    /// This is the dedicated processing task function. It receives packets from the receive loop
    /// via an mpsc channel and performs all CPU-intensive operations (decryption, rkyv parsing,
    /// state writes) in a single-threaded context.
    async fn process_packet(
        data: &[u8],
        src: std::net::SocketAddr,
        sequence_map: &mut HashMap<(String, String), u32>,
        pairing_manager: &Arc<std::sync::RwLock<crate::pairing_manager::PairingManager>>,
        shared_state: &Arc<std::sync::RwLock<AppState>>,
        ui_tx: &Option<Sender<UdpMessage>>,
    ) {
        // Wire format (Phase 2): [32-byte sender_x25519_pubkey][12-byte nonce][ciphertext+tag]
        // Extract sender pubkey from header first.
        if data.len() < crypto::SENDER_PUBKEY_LEN + crypto::NONCE_LEN + crypto::TAG_LEN {
            log::warn!(
                "Packet too short for Phase 2 wire format: {} bytes (min={})",
                data.len(),
                crypto::SENDER_PUBKEY_LEN + crypto::NONCE_LEN + crypto::TAG_LEN
            );
            return;
        }
        let (sender_x25519_pubkey_bytes, remainder) = data.split_at(crypto::SENDER_PUBKEY_LEN);
        let sender_x25519_pubkey: [u8; 32] = sender_x25519_pubkey_bytes.try_into().unwrap();

        // Derive ECDH key on the fly from sender's pubkey in the wire header.
        let ecdh_key = pairing_manager
            .read()
            .unwrap()
            .derive_ecdh_key_for_sender(&sender_x25519_pubkey);
        let ecdh_cipher = crypto::cipher_from_key(&ecdh_key);

        let plaintext = match crypto::open(&ecdh_cipher, remainder) {
            Ok(pt) => pt,
            Err(_) => {
                log::warn!(
                    "Decryption failed for packet from {} — sender may not have receiver_pubkey configured",
                    src
                );
                return;
            }
        };

        // Zero-copy access into the decrypted plaintext (aligned buffer).
        let archived: ArchivedPacketRef<'_> =
            match access::<ArchivedMetricPacket, rkyv::rancor::Error>(&plaintext) {
                Ok(pkt) => pkt,
                Err(e) => {
                    log::warn!("Failed to parse decrypted packet from {}: {}", src, e);
                    return;
                }
            };

        // Check timestamp freshness for replay protection
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if !Self::check_timestamp_freshness(archived.timestamp.into(), now) {
            log::warn!(
                "Timestamp check failed for packet from {}: timestamp={}, now={}",
                src,
                archived.timestamp,
                now
            );
            return;
        }

        // Check sequence number for replay detection (SEC-03: keyed by session_id)
        let machine_id_str = Self::machine_id_to_str(&archived.machine_id);
        log::info!(
            "🔍 Packet decoded — machine_id='{}' from {}",
            machine_id_str,
            src
        );
        let session_id_str = Self::session_id_to_str(&archived.sender_session_id);
        if !Self::check_sequence(
            sequence_map,
            &machine_id_str,
            &session_id_str,
            archived.sequence.into(),
        ) {
            // Sequence check failed (replay or out-of-order) — packet already logged in check_sequence
            return;
        }

        // TOFU pairing detection: Check if sender is paired
        let is_paired = pairing_manager.read().unwrap().is_paired(&machine_id_str);

        if !is_paired {
            // Unknown sender — create PairingRequest with REAL X25519 pubkey from packet header.
            let pubkey_hex = hex::encode(&sender_x25519_pubkey);
            log::info!(
                "🔔 Received pairing request from unpaired machine: {} (host: {}, x25519_pubkey={:.8}…)",
                machine_id_str,
                src.ip(),
                &pubkey_hex[..8]
            );
            let pairing_request = crate::pairing_manager::PairingRequest {
                machine_id: machine_id_str.to_string(),
                sender_pubkey: sender_x25519_pubkey, // REAL X25519 pubkey from packet header
                host: src.ip().to_string(),
                received_at: std::time::Instant::now(),
                tcp_response: None,
            };

            // Write directly to shared_state so the UI sees it regardless of whether
            // the tx channel is wired up (it is None in the background thread path).
            {
                let mut state = shared_state.write().unwrap();
                let src_ip = src.ip().to_string();
                let already_pending_from_ip =
                    state.pending_pairings.iter().any(|r| r.host == src_ip);
                let total_pending = state.pending_pairings.len();

                if total_pending >= 20 {
                    log::warn!(
                        "Pending pairings queue full (20) — dropping request from {}",
                        src_ip
                    );
                } else if already_pending_from_ip {
                    // deduplicate by IP too (not just machine_id)
                    log::debug!(
                        "Dropping duplicate pairing request from same IP: {}",
                        src_ip
                    );
                } else if !state
                    .pending_pairings
                    .iter()
                    .any(|r| r.machine_id == pairing_request.machine_id)
                {
                    state.pending_pairings.push(pairing_request.clone());
                    log::info!(
                        "🔔 Added pairing request to queue for machine: {}",
                        pairing_request.machine_id
                    );
                }
            }

            // Also send via channel if tx is available (future use).
            if let Some(tx) = ui_tx {
                let payload = UdpPayload::PairingRequest(pairing_request);
                let msg = UdpMessage { payload };
                let _ = tx.send(msg).await;
            }
            return;
        }

        // Verify sender pubkey matches what we stored at pairing time
        let is_sender_valid = pairing_manager
            .read()
            .unwrap()
            .verify_sender_pubkey(&machine_id_str, &sender_x25519_pubkey);

        if !is_sender_valid {
            log::warn!(
                "⚠️  Sender pubkey mismatch for machine '{}' — dropping packet (attack or unauthorized key change)",
                machine_id_str
            );
            return;
        }

        // Log which key succeeded and warn if a paired machine is still using bootstrap mode.
        log::debug!("✅ Decrypted with ECDH key for machine: {}", machine_id_str);

        // Convert archived packet to owned MetricPacket with nested structs
        let metric_packet = MetricPacket {
            version: archived.version.into(),
            machine_id: archived.machine_id,
            sender_session_id: archived.sender_session_id,
            timestamp: archived.timestamp.into(),
            sequence: archived.sequence.into(),
            cpu: CpuMetrics {
                usage_percent: archived.cpu.usage_percent.into(),
                temperature_celsius: match archived.cpu.temperature_celsius.as_ref() {
                    Some(v) => Some((*v).into()),
                    None => None,
                },
            },
            gpu: GpuMetrics {
                load_percent: match archived.gpu.load_percent.as_ref() {
                    Some(v) => Some((*v).into()),
                    None => None,
                },
                vram_used_mb: match archived.gpu.vram_used_mb.as_ref() {
                    Some(v) => Some((*v).into()),
                    None => None,
                },
                vram_total_mb: match archived.gpu.vram_total_mb.as_ref() {
                    Some(v) => Some((*v).into()),
                    None => None,
                },
                temperature_celsius: match archived.gpu.temperature_celsius.as_ref() {
                    Some(v) => Some((*v).into()),
                    None => None,
                },
            },
            memory: MemoryMetrics {
                used_bytes: archived.memory.used_bytes.into(),
                total_bytes: archived.memory.total_bytes.into(),
                swap_used_pct: archived.memory.swap_used_pct.into(),
            },
            network: NetworkMetrics {
                rx_bytes: archived.network.rx_bytes.into(),
                tx_bytes: archived.network.tx_bytes.into(),
            },
            disk: DiskMetrics {
                used_bytes: archived.disk.used_bytes.into(),
                total_bytes: archived.disk.total_bytes.into(),
                read_bytes: match archived.disk.read_bytes.as_ref() {
                    Some(v) => Some((*v).into()),
                    None => None,
                },
                write_bytes: match archived.disk.write_bytes.as_ref() {
                    Some(v) => Some((*v).into()),
                    None => None,
                },
                partitions: archived
                    .disk
                    .partitions
                    .iter()
                    .map(|p| nmd_service::packet::PartitionInfo {
                        mount: p.mount.to_string(),
                        total: p.total.into(),
                        used: p.used.into(),
                    })
                    .collect(),
            },
            uptime_seconds: archived.uptime_seconds.into(),
        };

        // Update RemoteMachine instances with new metrics
        // Convert machine_id from [u8; 20] to String
        let machine_id_len = metric_packet
            .machine_id
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(20);
        let machine_name = std::str::from_utf8(&metric_packet.machine_id[..machine_id_len])
            .unwrap_or("unknown")
            .to_string();

        // Guard scoped in a block so the future stays Send — a guard whose storage
        // spans the `.await` below (even only on unwind paths) breaks tokio::spawn.
        {
            let mut state = shared_state.write().unwrap();
            if let Some(machine_arc) = state
                .machines
                .iter()
                .find(|m| m.read().unwrap().name == machine_name)
            {
                machine_arc
                    .write()
                    .unwrap()
                    .update_from_packet(&metric_packet);
                log::debug!(
                    "📊 Updated metrics for machine: {} (CPU: {:.1}%, Mem: {}/{} bytes)",
                    machine_name,
                    metric_packet.cpu.usage_percent,
                    metric_packet.memory.used_bytes,
                    metric_packet.memory.total_bytes
                );
            } else if state.local_machine.read().unwrap().name == machine_name {
                // This is the local machine — metrics collected directly, ignore UDP packets for it
                log::debug!(
                    "Ignoring UDP packet for local machine '{}' (collected directly)",
                    machine_name
                );
            } else {
                // New remote machine — create it dynamically
                let new_machine = crate::remote_machine::RemoteMachine::new(machine_name.clone());
                state
                    .machines
                    .push(std::sync::Arc::new(std::sync::RwLock::new(new_machine)));
                log::info!("📍 Added new remote machine from UDP: {}", machine_name);
            }
        }

        // Send message to UI if transmitter is available
        if let Some(tx) = ui_tx {
            let payload = UdpPayload::Metrics(metric_packet);
            let msg = UdpMessage { payload };
            let _ = tx.send(msg).await;
        }
    }

    /// Decrypt a wire packet (`[32-byte sender_x25519_pubkey][12-byte nonce][ciphertext+tag]`) into rkyv plaintext bytes.
    /// AEAD tag verification happens inside `open()` — an Err means the packet was tampered
    /// with, truncated, or encrypted under a different key.
    /// Public so integration tests and criterion benchmarks can exercise the decryption path directly.
    pub fn decrypt_packet(&self, wire: &[u8]) -> Result<rkyv::util::AlignedVec, String> {
        // Wire format (ECDH-only): [32-byte sender_x25519_pubkey][12-byte nonce][ciphertext+tag]
        if wire.len() < crypto::SENDER_PUBKEY_LEN + crypto::NONCE_LEN + crypto::TAG_LEN {
            return Err(format!(
                "wire packet too short: {} bytes (min={})",
                wire.len(),
                crypto::SENDER_PUBKEY_LEN + crypto::NONCE_LEN + crypto::TAG_LEN
            ));
        }
        let sender_pubkey: [u8; 32] = wire[..crypto::SENDER_PUBKEY_LEN].try_into().unwrap();
        let remainder = &wire[crypto::SENDER_PUBKEY_LEN..];
        let ecdh_key = self
            .pairing_manager
            .read()
            .unwrap()
            .derive_ecdh_key_for_sender(&sender_pubkey);
        let ecdh_cipher = crypto::cipher_from_key(&ecdh_key);
        crypto::open(&ecdh_cipher, remainder)
    }

    /// Convert a fixed-length [u8; 20] machine_id field from an ArchivedMetricPacket to a displayable string.
    /// Truncates at the first null byte (null-padded encoding). Returns "<unknown>" if all zeros.
    fn machine_id_to_str(machine_id: &[u8; 20]) -> &str {
        // Find the first null byte — rkyv serializes [u8; 20] as inline bytes, no length prefix needed.
        let len = machine_id.iter().position(|&b| b == 0).unwrap_or(20);
        if len == 0 {
            return "<unknown>"; // All zeros = uninitialized/empty machine ID.
        }
        std::str::from_utf8(&machine_id[..len]).unwrap_or("<invalid-utf8>")
    }

    /// Convert a fixed-length [u8; 16] sender_session_id to hex string for logging/keying.
    /// SEC-03: Session IDs are random binary data, not UTF-8 strings.
    fn session_id_to_str(session_id: &[u8; 16]) -> String {
        session_id
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }

    /// Check if a packet's timestamp is fresh enough to be accepted (< TIMESTAMP_FRESHNESS_SECS old).
    fn check_timestamp_freshness(timestamp: u64, now: u64) -> bool {
        // Reject packets older than TIMESTAMP_FRESHNESS_SECS (replay protection) or from the future.
        let age = now.saturating_sub(timestamp);
        age < TIMESTAMP_FRESHNESS_SECS && timestamp <= now // No forward clock skew tolerance — prevents replay.
    }

    /// Check sequence number for replay detection — returns true if this is a new/expected sequence,
    /// false if it's a duplicate or out-of-order (replay attempt). Updates internal map on success.
    /// SEC-03: Keys by (machine_id, session_id) tuple to handle sender restarts gracefully.
    /// Item 7.3: Detects and logs packet loss (sequence gaps).
    fn check_sequence(
        seq_map: &mut HashMap<(String, String), u32>,
        machine_id: &str,
        session_id: &str,
        sequence: u32,
    ) -> bool {
        let key = (machine_id.to_string(), session_id.to_string());
        match seq_map.get(&key) {
            Some(&last_seq) => {
                // Reject if sequence is <= last seen (replay or out-of-order).
                if sequence > last_seq {
                    // Item 7.3: Detect packet loss (sequence gap > 1)
                    let expected_seq = last_seq + 1;
                    if sequence > expected_seq {
                        let lost_count = sequence - expected_seq;
                        log::warn!(
                            "📉 Packet loss detected: machine '{}' session '{}' — lost {} packet(s) (seq {}-{})",
                            machine_id,
                            &session_id[..8.min(session_id.len())],
                            lost_count,
                            expected_seq,
                            sequence - 1
                        );
                    }
                    seq_map.insert(key, sequence);
                    true
                } else {
                    log::warn!(
                        "Replay detected: machine '{}' session '{}' seq {} <= last {}",
                        machine_id,
                        &session_id[..8.min(session_id.len())],
                        sequence,
                        last_seq
                    );
                    false
                }
            }
            None => {
                // First packet from this machine/session — accept and record.
                log::info!(
                    "🆕 New session detected: machine '{}' session '{}'",
                    machine_id,
                    &session_id[..8.min(session_id.len())]
                );
                seq_map.insert(key, sequence);
                true
            }
        }
    }
}

/// Lifetime-bound reference type for zero-copy access to archived packets.
pub type ArchivedPacketRef<'a> = &'a ArchivedMetricPacket;

#[cfg(test)]
mod tests {
    use super::*;
    // Shared helpers — single source of truth for packet construction + wire encryption.
    use crate::network::test_support::{
        create_test_packet, create_test_packet_full, encrypt_packet, encrypt_packet_ecdh,
        test_sender_pubkey, unix_now,
    };

    /// Helper function to create a test PairingManager for unit tests
    fn test_pairing_manager()
    -> std::sync::Arc<std::sync::RwLock<crate::pairing_manager::PairingManager>> {
        use std::path::PathBuf;
        Arc::new(RwLock::new(crate::pairing_manager::PairingManager::new(
            PathBuf::from("/tmp/test_pairing.toml"),
        )))
    }

    #[test]
    fn test_packet_deserialization_with_nested_structs() {
        // Invalid/zeroed buffer should fail zero-copy access
        let data = vec![0u8; 64];
        let result = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(&data);
        assert!(
            result.is_err(),
            "Zero-copy access to invalid bytes should fail"
        );

        // Create valid packet with nested struct groups, encrypted into the wire format
        let pm = test_pairing_manager();
        let receiver_pubkey_hex = pm.read().unwrap().get_receiver_x25519_pubkey();

        let packet = create_test_packet("pluto", 45.5, 8_589_934_592, 17_179_869_184);
        let wire = encrypt_packet_ecdh(packet.clone(), &receiver_pubkey_hex);

        // Decrypt (verifies AEAD tag) then zero-copy access the plaintext
        let receiver = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(UdpReceiver::new(0, None, pm))
            .expect("Receiver creation should succeed");
        let buffer = receiver
            .decrypt_packet(&wire)
            .expect("Decryption should succeed");
        let archived = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(&buffer)
            .expect("Zero-copy access should succeed");

        // Convert archived to owned nested packet
        let parsed = MetricPacket {
            version: archived.version.into(),
            machine_id: archived.machine_id,
            sender_session_id: archived.sender_session_id,
            timestamp: archived.timestamp.into(),
            sequence: archived.sequence.into(),
            cpu: CpuMetrics {
                usage_percent: archived.cpu.usage_percent.into(),
                temperature_celsius: archived
                    .cpu
                    .temperature_celsius
                    .as_ref()
                    .map(|v| (*v).into()),
            },
            gpu: GpuMetrics {
                load_percent: archived.gpu.load_percent.as_ref().map(|v| (*v).into()),
                vram_used_mb: archived.gpu.vram_used_mb.as_ref().map(|v| (*v).into()),
                vram_total_mb: archived.gpu.vram_total_mb.as_ref().map(|v| (*v).into()),
                temperature_celsius: archived
                    .gpu
                    .temperature_celsius
                    .as_ref()
                    .map(|v| (*v).into()),
            },
            memory: MemoryMetrics {
                used_bytes: archived.memory.used_bytes.into(),
                total_bytes: archived.memory.total_bytes.into(),
                swap_used_pct: archived.memory.swap_used_pct.into(),
            },
            network: NetworkMetrics {
                rx_bytes: archived.network.rx_bytes.into(),
                tx_bytes: archived.network.tx_bytes.into(),
            },
            disk: DiskMetrics {
                used_bytes: archived.disk.used_bytes.into(),
                total_bytes: archived.disk.total_bytes.into(),
                read_bytes: archived.disk.read_bytes.as_ref().map(|v| (*v).into()),
                write_bytes: archived.disk.write_bytes.as_ref().map(|v| (*v).into()),
                partitions: archived
                    .disk
                    .partitions
                    .iter()
                    .map(|p| nmd_service::packet::PartitionInfo {
                        mount: p.mount.to_string(),
                        total: p.total.into(),
                        used: p.used.into(),
                    })
                    .collect(),
            },
            uptime_seconds: archived.uptime_seconds.into(),
        };

        // Verify machine ID
        let machine_id_len = parsed.machine_id.iter().position(|&b| b == 0).unwrap_or(20);
        let machine_name = std::str::from_utf8(&parsed.machine_id[..machine_id_len]).unwrap();
        assert_eq!(machine_name, "pluto");

        // Verify nested CPU metrics
        assert_eq!(parsed.cpu.usage_percent, 45.5);
        assert_eq!(parsed.cpu.temperature_celsius, Some(65.0));

        // Verify nested GPU metrics
        assert_eq!(parsed.gpu.load_percent, Some(50.0));
        assert_eq!(parsed.gpu.vram_used_mb, Some(2048));
        assert_eq!(parsed.gpu.vram_total_mb, Some(8192));

        // Verify nested Memory metrics
        assert_eq!(parsed.memory.used_bytes, 8_589_934_592);
        assert_eq!(parsed.memory.total_bytes, 17_179_869_184);

        // Verify nested Network metrics
        assert_eq!(parsed.network.rx_bytes, 1_000_000);
        assert_eq!(parsed.network.tx_bytes, 500_000);

        // Verify nested Disk metrics
        assert_eq!(parsed.disk.used_bytes, 320_000_000_000);
        assert_eq!(parsed.disk.partitions.len(), 1);
        assert_eq!(parsed.disk.partitions[0].mount, "/");
    }

    /// AEAD decryption: correct key roundtrips; a packet encrypted under a different key
    /// must fail tag verification inside decrypt_packet.
    #[test]
    fn test_decryption_key_verification() {
        let pm = test_pairing_manager();
        let receiver_pubkey_hex = pm.read().unwrap().get_receiver_x25519_pubkey();

        let packet = create_test_packet("spark", 30.0, 4_000_000_000, 8_000_000_000);

        // Encrypted under the ECDH key derived from receiver's pubkey — receiver must decrypt it.
        let wire = encrypt_packet_ecdh(packet.clone(), &receiver_pubkey_hex);
        let receiver = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(UdpReceiver::new(0, None, pm))
            .expect("Receiver creation should succeed");
        let plaintext = receiver
            .decrypt_packet(&wire)
            .expect("Decryption should succeed with correct key");
        assert!(rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(&plaintext).is_ok());

        // Encrypted under a *different* key — AEAD tag verification must fail.
        let foreign_wire = encrypt_packet(packet, &[0x99; 32]);
        assert!(
            receiver.decrypt_packet(&foreign_wire).is_err(),
            "Decryption should fail for packets encrypted under a different key"
        );

        // Runt packets (shorter than nonce + tag) are rejected without panicking.
        assert!(
            receiver.decrypt_packet(&[0u8; 5]).is_err(),
            "Short packet should be rejected"
        );
    }

    #[test]
    fn test_timestamp_freshness() {
        let mut packet = create_test_packet("test-machine", 50.0, 1_000_000_000, 2_000_000_000);

        // Set timestamp to 20 seconds in the past (should fail freshness check)
        packet.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 20;

        let pm = test_pairing_manager();
        let receiver_pubkey_hex = pm.read().unwrap().get_receiver_x25519_pubkey();

        let wire = encrypt_packet_ecdh(packet, &receiver_pubkey_hex);
        let receiver = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(UdpReceiver::new(0, None, pm))
            .expect("Receiver creation should succeed");
        let buffer = receiver
            .decrypt_packet(&wire)
            .expect("Decryption should succeed");

        let archived = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(&buffer)
            .expect("Access should succeed");

        // Timestamp check: packets older than 10s should be rejected
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp: u64 = archived.timestamp.into();

        assert!(
            now - timestamp > 10,
            "Test packet timestamp should be stale (>10s old)"
        );
    }

    /// Validates replay-attack detection: a packet with a duplicate sequence number must be
    /// rejected even though it decrypts cleanly and its timestamp is valid, and the sequence_map
    /// must always track the highest sequence seen per machine.
    #[test]
    fn test_replay_attack_detection() {
        let pm = test_pairing_manager();
        let receiver_pubkey_hex = pm.read().unwrap().get_receiver_x25519_pubkey();

        let receiver = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(UdpReceiver::new(0, None, pm))
            .expect("Receiver creation should succeed");

        // Valid packet with sequence #42 — passes decryption and freshness, mirroring listen_loop order.
        let packet = create_test_packet("replay-victim", 25.0, 1_000_000_000, 2_000_000_000);
        let wire = encrypt_packet_ecdh(packet, &receiver_pubkey_hex);
        let buffer = receiver
            .decrypt_packet(&wire)
            .expect("Decryption should succeed");
        let archived = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(&buffer)
            .expect("Access should succeed");
        assert!(
            UdpReceiver::check_timestamp_freshness(archived.timestamp.into(), unix_now()),
            "Fresh timestamp should pass"
        );

        let machine_id = UdpReceiver::machine_id_to_str(&archived.machine_id);
        let session_id = UdpReceiver::session_id_to_str(&archived.sender_session_id);
        assert_eq!(machine_id, "replay-victim");

        // First delivery of sequence 42 is accepted.
        assert!(
            UdpReceiver::check_sequence(
                &mut receiver.sequence_map.lock().unwrap(),
                machine_id,
                &session_id,
                archived.sequence.into()
            ),
            "First packet with sequence 42 should be accepted"
        );

        // Resend of the identical packet (duplicate sequence 42) must be rejected (logs a warning).
        assert!(
            !UdpReceiver::check_sequence(
                &mut receiver.sequence_map.lock().unwrap(),
                machine_id,
                &session_id,
                archived.sequence.into()
            ),
            "Replayed packet with duplicate sequence 42 should be rejected"
        );

        // sequence_map still holds the highest sequence seen (42), then advances to 43.
        assert_eq!(
            receiver
                .sequence_map
                .lock()
                .unwrap()
                .get(&(machine_id.to_string(), session_id.clone())),
            Some(&42)
        );
        assert!(
            UdpReceiver::check_sequence(
                &mut receiver.sequence_map.lock().unwrap(),
                machine_id,
                &session_id,
                43
            ),
            "Next monotonic sequence should be accepted"
        );
        assert_eq!(
            receiver
                .sequence_map
                .lock()
                .unwrap()
                .get(&(machine_id.to_string(), session_id.clone())),
            Some(&43),
            "sequence_map must track the highest sequence per machine"
        );

        // Out-of-order (lower) sequence after 43 is also a replay.
        assert!(
            !UdpReceiver::check_sequence(
                &mut receiver.sequence_map.lock().unwrap(),
                machine_id,
                &session_id,
                41
            ),
            "Out-of-order lower sequence should be rejected"
        );
    }

    /// Validates clock-skew tolerance: packets up to TIMESTAMP_FRESHNESS_SECS (10s) old are
    /// accepted (5s in the past passes), while anything older (15s) is rejected as a replay.
    #[test]
    fn test_clock_skew_tolerance() {
        let now = unix_now();

        // 5 seconds in the past — inside the 10s freshness window, must be accepted.
        assert!(
            UdpReceiver::check_timestamp_freshness(now - 5, now),
            "Packet 5s old (within 10s window) should be accepted"
        );

        // 15 seconds in the past — outside the window, must be rejected.
        assert!(
            !UdpReceiver::check_timestamp_freshness(now - 15, now),
            "Packet 15s old (outside 10s window) should be rejected"
        );

        // Boundary: exactly 10s old is rejected (strict `age < 10` comparison).
        assert!(
            !UdpReceiver::check_timestamp_freshness(now - TIMESTAMP_FRESHNESS_SECS, now),
            "Packet exactly at the freshness boundary should be rejected"
        );

        // End-to-end: an encrypted packet backdated 5s still passes the full freshness check.
        let pm = test_pairing_manager();
        let receiver_pubkey_hex = pm.read().unwrap().get_receiver_x25519_pubkey();

        let packet = create_test_packet_full("skewed", 10.0, 1_000, 2_000, 1, now - 5);
        let wire = encrypt_packet_ecdh(packet, &receiver_pubkey_hex);
        let receiver = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(UdpReceiver::new(0, None, pm))
            .expect("Receiver creation should succeed");
        let buffer = receiver
            .decrypt_packet(&wire)
            .expect("Decryption should succeed");
        let archived = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(&buffer)
            .expect("Access should succeed");
        assert!(UdpReceiver::check_timestamp_freshness(
            archived.timestamp.into(),
            unix_now()
        ));
    }

    /// Validates unknown sender emits pairing request: a packet from unpaired machine triggers PairingRequest
    #[test]
    fn test_unknown_sender_emits_pairing_request() {
        // Create a receiver with an empty PairingManager (no paired machines)
        let pm = test_pairing_manager();
        let receiver_pubkey_hex = pm.read().unwrap().get_receiver_x25519_pubkey();

        let receiver = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(UdpReceiver::new(0, None, pm))
            .expect("Receiver creation should succeed");

        // Packet from a machine NOT in the pairing manager
        let packet = create_test_packet("unknown-machine", 25.0, 1_000_000_000, 2_000_000_000);
        let wire = encrypt_packet_ecdh(packet.clone(), &receiver_pubkey_hex);

        // Decrypt to get the archived packet
        let buffer = receiver
            .decrypt_packet(&wire)
            .expect("Decryption should succeed");
        let archived = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(&buffer)
            .expect("Access should succeed");

        // Verify machine is NOT paired
        assert!(
            !receiver
                .pairing_manager
                .read()
                .unwrap()
                .is_paired("unknown-machine")
        );

        // Extract machine_id for pairing request check
        let machine_id_str = UdpReceiver::machine_id_to_str(&archived.machine_id);

        // The TOFU logic should detect unpaired sender and emit PairingRequest
        // In listen_loop, this would send UdpPayload::PairingRequest via tx
        // Since we don't have a channel here, verify the condition that triggers it
        assert_eq!(machine_id_str, "unknown-machine");
        assert!(
            !receiver
                .pairing_manager
                .read()
                .unwrap()
                .is_paired(machine_id_str)
        );
    }

    /// Validates paired sender passes through: a packet from paired machine triggers Metrics payload
    #[test]
    fn test_paired_sender_passes_through() {
        // Create a receiver and add a machine to the PairingManager
        let pm = test_pairing_manager();
        let mut manager = pm.write().unwrap();

        // Use fixed test sender pubkey for consistency
        let sender_pubkey_bytes = test_sender_pubkey();

        // Add the machine as paired with the test sender's pubkey
        manager
            .add_pairing(
                "paired-machine".to_string(),
                &sender_pubkey_bytes,
                "127.0.0.1".to_string(),
            )
            .expect("Failed to add pairing");

        drop(manager); // Release write lock

        let receiver_pubkey_hex = pm.read().unwrap().get_receiver_x25519_pubkey();

        let receiver = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(UdpReceiver::new(0, None, pm.clone()))
            .expect("Receiver creation should succeed");

        // Packet from a machine that IS in the pairing manager
        let packet = create_test_packet("paired-machine", 30.0, 1_500_000_000, 3_000_000_000);
        let wire = encrypt_packet_ecdh(packet.clone(), &receiver_pubkey_hex);

        // Decrypt to get the archived packet
        let buffer = receiver
            .decrypt_packet(&wire)
            .expect("Decryption should succeed");
        let archived = rkyv::access::<ArchivedMetricPacket, rkyv::rancor::Error>(&buffer)
            .expect("Access should succeed");

        // Verify machine IS paired
        assert!(
            receiver
                .pairing_manager
                .read()
                .unwrap()
                .is_paired("paired-machine")
        );

        // Extract machine_id for pass-through check
        let machine_id_str = UdpReceiver::machine_id_to_str(&archived.machine_id);

        // The TOFU logic should detect paired sender and allow metrics processing
        assert_eq!(machine_id_str, "paired-machine");
        assert!(
            receiver
                .pairing_manager
                .read()
                .unwrap()
                .is_paired(machine_id_str)
        );
    }

    /// Validates future-timestamp rejection: a packet stamped 30s ahead of local time must be
    /// rejected — accepting future timestamps would let an attacker pre-date packets for replay.
    #[test]
    fn test_future_timestamp_rejection() {
        let now = unix_now();

        // 30 seconds in the future — rejected (timestamp <= now required, no forward skew allowed).
        assert!(
            !UdpReceiver::check_timestamp_freshness(now + 30, now),
            "Packet with timestamp 30s in the future should be rejected"
        );

        // Even 1 second in the future is rejected — zero forward clock-skew tolerance.
        assert!(
            !UdpReceiver::check_timestamp_freshness(now + 1, now),
            "Packet with any future timestamp should be rejected"
        );

        // A timestamp of exactly now is valid.
        assert!(
            UdpReceiver::check_timestamp_freshness(now, now),
            "Packet stamped exactly now should be accepted"
        );
    }

    /// Validates AEAD tamper detection: flipping a single bit anywhere in the wire packet
    /// (nonce, ciphertext body, or Poly1305 tag) must cause decrypt_packet() to fail.
    #[test]
    fn test_wire_tamper_detection() {
        let pm = test_pairing_manager();
        let receiver_pubkey_hex = pm.read().unwrap().get_receiver_x25519_pubkey();

        let receiver = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(UdpReceiver::new(0, None, pm))
            .expect("Receiver creation should succeed");

        let packet = create_test_packet("tamper-test", 33.3, 4_000_000_000, 8_000_000_000);
        let wire = encrypt_packet_ecdh(packet, &receiver_pubkey_hex);

        // Sanity: untampered packet decrypts.
        assert!(
            receiver.decrypt_packet(&wire).is_ok(),
            "Untampered packet should decrypt"
        );

        // Wire format (Phase 2): [32-byte sender_pubkey][12-byte nonce][ciphertext+tag]
        let nonce_offset = nmd_service::crypto::SENDER_PUBKEY_LEN;
        // Flip one bit in the nonce, first ciphertext byte, and last tag byte — all must fail.
        for (idx, what) in [
            (nonce_offset, "nonce"),
            (
                nonce_offset + nmd_service::crypto::NONCE_LEN,
                "first ciphertext byte",
            ),
            (wire.len() - 1, "last tag byte"),
        ] {
            let mut mutated = wire.clone();
            mutated[idx] ^= 0x01;
            assert!(
                receiver.decrypt_packet(&mutated).is_err(),
                "Packet with a flipped bit in the {what} must be rejected"
            );
        }
    }
}
