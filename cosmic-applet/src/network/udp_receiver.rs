#!/ # UdpReceiver — Listens for rkyv-encoded MetricPacket via UDP with HMAC-SHA256 verification
//!
//! Binds to a configurable port and receives incoming UDP packets from remote nmd-service instances.
//! Each packet is verified via HMAC-SHA256 using the pre-shared key, then checked for replay protection:
//! timestamp freshness (< 10s old) + monotonic sequence number tracking per machine_id (per Worf's security analysis).
use subtle::ConstantTimeEq;
use hmac::{Hmac, KeyInit, Mac};
// ConstantTimeEq is imported from `subtle` (v2) which provides the same ct_eq API.
// Per Worf's security audit VULN-01: hmac 0.13 re-exports from crypto-common/digest, not its own crypto_mac module.
use sha2::Sha256;
use std::cell::RefCell;
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::Sender;

// Re-use the same rkyv-serialized MetricPacket type from nmd-service's packet definition.
// Since both crates are in the same workspace, we import the struct directly.
use crate::AppState;
use nmd_service::packet::{MetricPacket, CpuMetrics, GpuMetrics, MemoryMetrics, NetworkMetrics, DiskMetrics};
use nmd_service::packet_flat::{ArchivedMetricPacketFlat, MetricPacketFlat};
use rkyv::access;

/// HMAC-SHA256 type alias for ergonomic use throughout the module.
type HmacSha256 = Hmac<Sha256>;

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

/// UDP receiver that listens for authenticated MetricPacket traffic from remote machines.
///
/// Maintains a per-machine sequence number map to detect replayed or out-of-order packets (Worf Phase 1A).
/// Updates the shared [`AppState`] grid window in real-time as new data arrives via async background task.
pub struct UdpReceiver {
    /// Bound UDP socket listening for incoming MetricPacket traffic from remote nmd-service instances.
    pub socket: UdpSocket,

    /// Pre-shared HMAC key loaded from /etc/nmd/secret.key (32 bytes) — used to verify packet authenticity.
    pub secret_key: Vec<u8>,

    /// Per-machine sequence number map for replay detection — maps machine_id → last seen sequence.
    /// Uses RefCell for interior mutability since the receive loop accesses it through `&self`.
    pub sequence_map: RefCell<HashMap<String, u32>>,

    /// Port the receiver is listening on (default: 51057).
    pub port: u16,

    /// Sender for sending metric updates to the UI (iced application).
    ///
    /// This allows the UDP receiver to send structured messages back to the main application thread
    /// via an async channel, enabling typed communication of received metrics.
    tx: Option<Sender<UdpMessage>>,
}

impl UdpReceiver {
    /// Load HMAC secret key from file.
    fn load_secret_key(path: &str) -> Result<Vec<u8>, std::io::Error> {
        use std::fs;
        let key_bytes = fs::read(path)?;
        if key_bytes.len() != 32 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("HMAC key must be exactly 32 bytes, got {}", key_bytes.len()),
            ));
        }
        Ok(key_bytes)
    }

    /// Create a new UDP receiver bound to the specified port with the given HMAC secret key.
    /// The socket binds to `0.0.0.0:port` to listen on all interfaces for incoming remote machine traffic.
    ///
    /// # Arguments
    ///
    /// * `port` - UDP port to bind to
    /// * `secret_key` - 32-byte HMAC-SHA256 key for packet verification
    /// * `tx` - Optional sender for communicating with the UI (if None, no messages will be sent)
    pub fn new(port: u16, secret_key: Vec<u8>, tx: Option<Sender<UdpMessage>>) -> Result<Self, std::io::Error> {
        let addr = format!("0.0.0.0:{}", port);
        log::info!("Binding UDP receiver to {}", addr);

        // Set socket read timeout so the receive loop can check for shutdown periodically.
        let socket = UdpSocket::bind(&addr)?;
        socket.set_read_timeout(Some(std::time::Duration::from_millis(500)))?;

        Ok(UdpReceiver {
            socket,
            secret_key,
            sequence_map: RefCell::new(HashMap::new()),
            port,
            tx,
        })
    }

    /// Start the async UDP receive loop — runs as a background tokio task.
    /// Continuously reads packets, verifies HMAC + freshness, and updates shared AppState grid window.
    ///
    /// # Arguments
    ///
    /// * `shared_state` - Shared application state that gets updated with received metrics
    pub async fn start_listening(shared_state: Arc<RwLock<AppState>>) {
        log::info!("🔌 Starting UDP receiver...");
        
        // Load configuration from shared_state
        let config_manager = shared_state.read().unwrap().config_manager.clone();
        let config_guard = config_manager.read().unwrap();
        let port = config_guard.udp_port; 
        let secret_key_path = config_guard.hmac_secret_path.clone();
        
        log::info!("UDP receiver config: port={}, secret_key_path={}", port, secret_key_path);

        // Load secret key
        let secret_key = match Self::load_secret_key(&secret_key_path) {
            Ok(key) => {
                log::info!("✓ Loaded HMAC secret key ({} bytes)", key.len());
                key
            }
            Err(e) => {
                log::error!("Failed to load HMAC secret key: {}", e);
                return;
            }
        };

        // Create socket
        let socket = match UdpSocket::bind(&format!("0.0.0.0:{}", port)) {
            Ok(sock) => {
                log::info!("✓ Bound UDP socket to 0.0.0.0:{}", port);
                sock
            }
            Err(e) => {
                log::error!("Failed to bind UDP socket: {}", e);
                return;
            }
        };
        let _ = socket.set_read_timeout(Some(std::time::Duration::from_millis(500)));

        // Create the receiver
        let mut receiver = UdpReceiver {
            socket,
            secret_key,
            sequence_map: RefCell::new(HashMap::new()),
            port,
            tx: None, // We don't have a way to send messages back in this context? 
                      // But note: the original design didn't use tx for sending to UI, it updated shared state directly.
        };

        log::info!("🎧 UDP receiver ready — waiting for packets...");
        
        // Run the listen loop
        receiver.listen_loop(shared_state).await;
    }

    /// Listen loop that processes incoming UDP packets.
    ///
    /// # Arguments
    ///
    /// * `shared_state` - Shared application state that gets updated with received metrics
    pub async fn listen_loop(&mut self, shared_state: Arc<RwLock<AppState>>) {
        loop {
            let mut buf = [0u8; MAX_PACKET_SIZE];
            match self.socket.recv_from(&mut buf) {
                Ok((size, src)) => {
                    let data = &buf[..size];

                    // Verify HMAC authenticity first (critical security step)
                    // We need to access the archived packet to verify HMAC
                    let archived: ArchivedPacketRef<'_> = match access::<ArchivedMetricPacketFlat, rkyv::rancor::Error>(data) {
                        Ok(pkt) => pkt,
                        Err(e) => {
                            log::warn!("Failed to parse packet from {} (pre-HMAC): {}", src, e);
                            continue;
                        }
                    };

                    if !self.verify_hmac_zerocopy(data, &archived) {
                        log::warn!("HMAC verification failed for packet from {}", src);
                        continue;
                    }

                    // Check timestamp freshness for replay protection
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    if !Self::check_timestamp_freshness(archived.timestamp.into(), now) {
                        log::warn!("Timestamp check failed for packet from {}: timestamp={}, now={}", src, archived.timestamp, now);
                        continue;
                    }

                    // Check sequence number for replay detection
                    let machine_id_str = Self::machine_id_to_str(&archived.machine_id);
                    if !Self::check_sequence(&self.sequence_map, &machine_id_str, archived.sequence.into()) {
                        // Sequence check failed (replay or out-of-order) — packet already logged in check_sequence
                        continue;
                    }

                    // Convert archived flat packet to owned MetricPacketFlat, then to nested MetricPacket
                    let flat_packet = MetricPacketFlat {
                        version: archived.version.into(),
                        machine_id: archived.machine_id,
                        timestamp: archived.timestamp.into(),
                        sequence: archived.sequence.into(),
                        
                        cpu_usage_percent: archived.cpu_usage_percent.into(),
                        cpu_temperature_celsius: match archived.cpu_temperature_celsius.as_ref() { 
                            Some(v) => Some((*v).into()), None => None 
                        },
                        
                        gpu_load_percent: match archived.gpu_load_percent.as_ref() { 
                            Some(v) => Some((*v).into()), None => None 
                        },
                        gpu_vram_used_mb: match archived.gpu_vram_used_mb.as_ref() { 
                            Some(v) => Some((*v).into()), None => None 
                        },
                        gpu_vram_total_mb: match archived.gpu_vram_total_mb.as_ref() { 
                            Some(v) => Some((*v).into()), None => None 
                        },
                        gpu_temperature_celsius: match archived.gpu_temperature_celsius.as_ref() { 
                            Some(v) => Some((*v).into()), None => None 
                        },
                        
                        memory_used_bytes: archived.memory_used_bytes.into(),
                        memory_total_bytes: archived.memory_total_bytes.into(),
                        memory_swap_used_pct: archived.memory_swap_used_pct.into(),
                        
                        network_rx_bytes: archived.network_rx_bytes.into(),
                        network_tx_bytes: archived.network_tx_bytes.into(),
                        
                        disk_used_bytes: archived.disk_used_bytes.into(),
                        disk_total_bytes: archived.disk_total_bytes.into(),
                        disk_read_bytes: match archived.disk_read_bytes.as_ref() { 
                            Some(v) => Some((*v).into()), None => None 
                        },
                        disk_write_bytes: match archived.disk_write_bytes.as_ref() { 
                            Some(v) => Some((*v).into()), None => None 
                        },
                        disk_partitions: archived.disk_partitions.iter().map(|p| {
                            nmd_service::packet::PartitionInfo {
                                mount: p.mount.to_string(),
                                total: p.total.into(),
                                used: p.used.into(),
                            }
                        }).collect(),
                        
                        uptime_seconds: archived.uptime_seconds.into(),
                        hmac_tag: archived.hmac_tag,
                    };
                    
                    // Convert flat packet to nested structure for clean API
                    let metric_packet = flat_packet.to_nested();

                    // Update RemoteMachine instances with new metrics
                    // Convert machine_id from [u8; 20] to String
                    let machine_id_len = metric_packet.machine_id.iter().position(|&b| b == 0).unwrap_or(20);
                    let machine_name = std::str::from_utf8(&metric_packet.machine_id[..machine_id_len])
                        .unwrap_or("unknown")
                        .to_string();
                    
                    let mut state = shared_state.write().unwrap();
                    if let Some(machine) = state.machines.get_mut(&machine_name) {
                        machine.update_from_packet(&metric_packet);
                        log::debug!("📊 Updated metrics for machine: {} (CPU: {:.1}%, Mem: {}/{} bytes)", 
                            machine_name, metric_packet.cpu.usage_percent, metric_packet.memory.used_bytes, metric_packet.memory.total_bytes);
                    } else {
                        // Machine not in config, create it dynamically
                        let mut new_machine = crate::remote_machine::RemoteMachine::new(machine_name.clone());
                        new_machine.update_from_packet(&metric_packet);
                        state.machines.insert(machine_name.clone(), new_machine);
                        log::info!("📍 Added new machine from UDP: {}", machine_name);
                    }
                    drop(state);

                    // Send message to UI if transmitter is available
                    if let Some(ref tx) = self.tx {
                        let payload = UdpPayload::Metrics(metric_packet);
                        let msg = UdpMessage { payload };
                        let _ = tx.send(msg).await;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {
                    // No packet received within timeout — continue loop (non-blocking).
                    tokio::task::yield_now().await;
                }
                Err(e) => {
                    log::error!("UDP receive error: {}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }

            // Offline detection disabled - machines remain visible until they send packets again
            // TODO: Add optional timeout-based offline marking if needed
        }
    }

    /// Verify HMAC-SHA256 tag using zero-copy buffer access — no re-serialization needed!
    /// Copies the received buffer once, zeroes the hmac_tag region in-place, computes HMAC over those bytes.
    fn verify_hmac_zerocopy(&self, buf: &[u8], archived: &ArchivedPacketRef<'_>) -> bool {
        // The received buffer IS the canonical serialized form (rkyv::access gave us a typed ref into it).
        // Copy once for HMAC computation — we zero out the tag region in this copy.
        let mut hmac_buf = buf.to_vec();

        // Compute the actual byte offset of hmac_tag field using pointer arithmetic.
        // archived is a reference into buf, so we can calculate the offset of archived.hmac_tag.
        let buf_ptr = buf.as_ptr() as usize;
        let tag_ptr = archived.hmac_tag.as_ptr() as usize;
        let tag_offset = tag_ptr - buf_ptr;
        
        log::debug!("  Tag offset in buffer: {} (buffer len: {})", tag_offset, buf.len());
        
        // Zero out the hmac_tag field's bytes in the copy at the correct offset.
        hmac_buf[tag_offset..tag_offset + 32].copy_from_slice(&[0u8; 32]);

        // Compute HMAC over the zeroed-tag buffer and compare with the received tag using constant-time comparison.
        let mut mac = HmacSha256::new_from_slice(&self.secret_key)
            .expect("HMAC key length is valid for SHA-256");
        mac.update(&hmac_buf);
        let computed_tag = mac.finalize().into_bytes();

        // DEBUG: Log received vs computed tags for diagnostics
        log::debug!("🔐 HMAC verification:");
        log::debug!("  Secret key (first 8 bytes): {:02x?}", &self.secret_key[..8.min(self.secret_key.len())]);
        log::debug!("  Received tag:  {:02x?}", &archived.hmac_tag[..8]);
        log::debug!("  Computed tag:  {:02x?}", &computed_tag[..8]);

        // Constant-time comparison to prevent timing attacks (Worf VULN-01).
        // hmac_tag is [u8; 32] in both MetricPacket and ArchivedMetricPacket, no dereference needed.
        let result: bool = ConstantTimeEq::ct_eq(&computed_tag[..], &archived.hmac_tag).into();
        
        if !result {
            log::warn!("  ❌ Tags don't match!");
        } else {
            log::debug!("  ✓ Tags match");
        }
        
        result
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

    /// Check if a packet's timestamp is fresh enough to be accepted (< TIMESTAMP_FRESHNESS_SECS old).
    fn check_timestamp_freshness(timestamp: u64, now: u64) -> bool {
        // Reject packets older than TIMESTAMP_FRESHNESS_SECS (replay protection) or from the future.
        let age = now.saturating_sub(timestamp);
        age < TIMESTAMP_FRESHNESS_SECS && timestamp <= now  // No forward clock skew tolerance — prevents replay.
    }

    /// Check sequence number for replay detection — returns true if this is a new/expected sequence,
    /// false if it's a duplicate or out-of-order (replay attempt). Updates internal map on success.
    fn check_sequence(seq_map: &RefCell<HashMap<String, u32>>, machine_id: &str, sequence: u32) -> bool {
        let mut map = seq_map.borrow_mut();
        match map.get(machine_id) {
            Some(&last_seq) => {
                // Reject if sequence is <= last seen (replay or out-of-order).
                if sequence > last_seq {
                    map.insert(machine_id.to_string(), sequence);
                    true
                } else {
                    log::warn!("Replay detected: machine '{}' seq {} <= last {}", machine_id, sequence, last_seq);
                    false
                }
            }
            None => {
                // First packet from this machine — accept and record.
                map.insert(machine_id.to_string(), sequence);
                true
            }
        }
    }
}

// Define a lifetime-bound reference type for zero-copy access to archived packets.
type ArchivedPacketRef<'a> = &'a ArchivedMetricPacketFlat;

#[cfg(test)]
mod tests {
    use super::*;

    /// Correctly parses MetricPacket from UDP bytes via rkyv zero-copy access.
    #[test]
    fn test_parse_incoming_packet() {
        // Invalid/zeroed buffer should fail to parse (not valid rkyv data).
        let data = vec![0u8; 64];
        let result = UdpReceiver::parse_packet(&data);
        assert!(result.is_err(), "Parsing invalid bytes should return Err");

        // Valid serialized packet should parse successfully via zero-copy access.
        let mut machine_id_bytes = [0u8; 20];
        let src = "pluto".as_bytes();
        let len = src.len().min(20);
        machine_id_bytes[..len].copy_from_slice(&src[..len]);
        
        let packet = nmd_service::packet::MetricPacket {
            version: nmd_service::packet::PROTOCOL_VERSION,
            machine_id: machine_id_bytes,
            timestamp: 100,
            sequence: 5,
            cpu_usage: 42.5,
            memory_used_bytes: 10_000_000_000,
            memory_total_bytes: 16_000_000_000,
            disk_used_bytes: 400_000_000_000,
            disk_total_bytes: 500_000_000_000,
            network_rx_bytes: 1_000_000,
            network_tx_bytes: 500_000,
            uptime_seconds: 3600,
            disk_read_bytes: None,      // Phase 2: IO stats (sysinfo doesn't expose these)
            disk_write_bytes: None,     // Phase 2: IO stats (sysinfo doesn't expose these)
            memory_swap_used_pct: 0.0,
            disk_partitions: Vec::new(),
            gpu_vram_used_mb: None,
            gpu_vram_total_mb: None,
            // Phase 2.1: GPU load percentage (optional)
            gpu_load_percent: None,
            temperature_celsius: None,
            gpu_temperature_celsius: None,
            hmac_tag: [0u8; 32],
        };

        let buf = rkyv::to_bytes::<rkyv::rancor::Error>(&packet).unwrap();
        let result = UdpReceiver::parse_packet(&buf);
        assert!(result.is_ok(), "Valid packet should parse successfully");
        let archived = result.unwrap();
        assert_eq!(archived.version, nmd_service::packet::PROTOCOL_VERSION);
        assert_eq!(archived.timestamp, 100);
        assert_eq!(archived.sequence, 5);
        assert_eq!(f32::from(archived.cpu_usage), 42.5);
        assert_eq!(u64::from(archived.memory_used_bytes), 10_000_000_000);
    }
}