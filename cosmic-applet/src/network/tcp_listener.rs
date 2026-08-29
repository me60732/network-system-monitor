//! TCP pairing listener — handles initial pairing handshake and key rotation requests.
//!
//! Runs alongside the UDP receiver on the same port (TCP and UDP are independent).
//! Pairing flow: sender connects → sends PairingHello → receiver queues request →
//! user accepts/denies in UI → response sent back over TCP → connection closes.
//! Key rotation: sender sends authenticated KeyRotation → receiver auto-accepts → ack sent.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::AppState;
use crate::pairing_manager::{PairingRequest, TcpPairingResponse};

/// Start the TCP pairing listener on the given port.
/// Spawns a tokio task that accepts connections and handles each in its own task.
pub async fn start_tcp_listener(port: u16, shared_state: Arc<RwLock<AppState>>) {
    let addr = format!("0.0.0.0:{}", port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => {
            log::info!("🔌 TCP pairing listener bound on {}", addr);
            l
        }
        Err(e) => {
            log::error!("Failed to bind TCP pairing listener on {}: {}", addr, e);
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                log::info!("🔌 TCP pairing connection from {}", peer_addr);
                let state_clone = Arc::clone(&shared_state);
                tokio::spawn(async move {
                    handle_tcp_connection(stream, peer_addr.ip().to_string(), state_clone).await;
                });
            }
            Err(e) => {
                log::error!("TCP accept error: {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

/// Handle a single TCP pairing connection.
async fn handle_tcp_connection(
    mut stream: tokio::net::TcpStream,
    peer_ip: String,
    shared_state: Arc<RwLock<AppState>>,
) {
    // Set 30-second read timeout
    stream.set_nodelay(true).ok();

    // Read length-prefixed JSON message
    let msg = match read_message(&mut stream).await {
        Ok(m) => m,
        Err(e) => {
            log::warn!("TCP read error from {}: {}", peer_ip, e);
            return;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&msg) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("TCP JSON parse error from {}: {}", peer_ip, e);
            return;
        }
    };

    match parsed.get("type").and_then(|t| t.as_str()) {
        Some("hello") => handle_pairing_hello(parsed, peer_ip, stream, shared_state).await,
        Some("rotate") => handle_key_rotation(parsed, peer_ip, stream, shared_state).await,
        Some(other) => log::warn!("Unknown TCP message type '{}' from {}", other, peer_ip),
        None => log::warn!("TCP message missing 'type' from {}", peer_ip),
    }
}

/// Handle a PairingHello — queue the request and wait for user decision (up to 120s).
async fn handle_pairing_hello(
    msg: serde_json::Value,
    peer_ip: String,
    mut stream: tokio::net::TcpStream,
    shared_state: Arc<RwLock<AppState>>,
) {
    use tokio::io::AsyncWriteExt;

    let machine_id = match msg.get("machine_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() && id.len() <= 20 => id.to_string(),
        _ => {
            log::warn!("PairingHello missing/invalid machine_id from {}", peer_ip);
            return;
        }
    };

    let sender_pubkey_hex = match msg.get("sender_pubkey").and_then(|v| v.as_str()) {
        Some(h) if h.len() == 64 => h.to_string(),
        _ => {
            log::warn!(
                "PairingHello missing/invalid sender_pubkey from {}",
                peer_ip
            );
            return;
        }
    };

    let sender_pubkey_bytes = match hex::decode(&sender_pubkey_hex) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => {
            log::warn!("sender_pubkey decode failed from {}", peer_ip);
            return;
        }
    };

    log::info!("🔔 TCP PairingHello from '{}' ({})", machine_id, peer_ip);

    // Create response channel
    let (tx, rx) = std::sync::mpsc::channel::<TcpPairingResponse>();
    let arc_tx = Arc::new(std::sync::Mutex::new(Some(tx)));

    let request = PairingRequest {
        machine_id: machine_id.clone(),
        sender_pubkey: sender_pubkey_bytes,
        host: peer_ip.clone(),
        received_at: std::time::Instant::now(),
        tcp_response: Some(arc_tx),
    };

    // Push to pending_pairings — check DoS limits
    let should_push = {
        let state = shared_state.write().unwrap();
        let already_pending = state
            .pending_pairings
            .iter()
            .any(|r| r.machine_id == request.machine_id || r.host == peer_ip);
        let queue_full = state.pending_pairings.len() >= 20;
        drop(state); // release lock before await
        !already_pending && !queue_full
    };

    if should_push {
        let mut state = shared_state.write().unwrap();
        state.pending_pairings.push(request);
        drop(state); // release lock before await
    } else {
        log::warn!(
            "Dropping TCP pairing from '{}' — queue full or duplicate",
            machine_id
        );
        let deny = serde_json::json!({"type":"deny"});
        let _ = write_message_sync_on_stream(&mut stream, &deny.to_string()).await;
        return;
    }

    // Wait up to 120s for user decision
    let response = tokio::task::spawn_blocking(move || {
        rx.recv_timeout(Duration::from_secs(120))
            .unwrap_or(TcpPairingResponse::Deny)
    })
    .await
    .unwrap_or(TcpPairingResponse::Deny);

    let reply = match response {
        TcpPairingResponse::Accept(receiver_pubkey) => {
            log::info!("✅ Sending PairingAccept to '{}' via TCP", machine_id);
            serde_json::json!({"type": "accept", "receiver_pubkey": receiver_pubkey})
        }
        TcpPairingResponse::Deny => {
            log::info!("❌ Sending PairingDeny to '{}' via TCP", machine_id);
            serde_json::json!({"type": "deny"})
        }
    };

    let _ = write_message_sync_on_stream(&mut stream, &reply.to_string()).await;
    let _ = stream.flush().await;
}

/// Handle a KeyRotation request — auto-accepts if authenticated with old ECDH key.
async fn handle_key_rotation(
    msg: serde_json::Value,
    peer_ip: String,
    mut stream: tokio::net::TcpStream,
    shared_state: Arc<RwLock<AppState>>,
) {
    use tokio::io::AsyncWriteExt;

    let machine_id = match msg.get("machine_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => {
            log::warn!("KeyRotation missing machine_id from {}", peer_ip);
            return;
        }
    };

    let nonce_hex = match msg.get("nonce").and_then(|v| v.as_str()) {
        Some(h) if h.len() == 24 => h,
        _ => {
            log::warn!("KeyRotation invalid nonce from {}", peer_ip);
            return;
        }
    };

    let ciphertext_hex = match msg.get("ciphertext").and_then(|v| v.as_str()) {
        Some(h) => h,
        None => {
            log::warn!("KeyRotation missing ciphertext from {}", peer_ip);
            return;
        }
    };

    let nonce_bytes = match hex::decode(nonce_hex) {
        Ok(b) if b.len() == 12 => {
            let mut a = [0u8; 12];
            a.copy_from_slice(&b);
            a
        }
        _ => {
            log::warn!("KeyRotation nonce decode failed from {}", peer_ip);
            return;
        }
    };

    let ciphertext = match hex::decode(ciphertext_hex) {
        Ok(b) => b,
        Err(_) => {
            log::warn!("KeyRotation ciphertext decode failed from {}", peer_ip);
            return;
        }
    };

    // Get pairing manager and derive old ECDH key for this machine
    let ecdh_key: Option<[u8; 32]> = {
        let state = shared_state.read().unwrap();
        let pm = state.pairing_manager.read().unwrap();
        if !pm.is_paired(&machine_id) {
            None
        } else {
            let old_sender_pubkey = pm.get_sender_pubkey(&machine_id).copied();
            Some(
                old_sender_pubkey
                    .map(|pk| pm.derive_ecdh_key_for_sender(&pk))
                    .unwrap_or([0u8; 32]),
            )
        }
    };

    if ecdh_key.is_none() {
        log::warn!(
            "KeyRotation for unknown machine '{}' from {}",
            machine_id,
            peer_ip
        );
        let deny = serde_json::json!({"type":"rotation_denied","reason":"unknown machine"});
        let _ = write_message_sync_on_stream(&mut stream, &deny.to_string()).await;
        return;
    }

    let ecdh_key = ecdh_key.unwrap();

    // Decrypt the new sender pubkey using old ECDH key
    use chacha20poly1305::{
        ChaCha20Poly1305, Nonce,
        aead::{Aead, KeyInit},
    };
    let cipher = ChaCha20Poly1305::new((&ecdh_key).into());
    let nonce = Nonce::from_slice(&nonce_bytes);

    let new_pubkey_bytes = match cipher.decrypt(nonce, ciphertext.as_ref()) {
        Ok(pt) if pt.len() == 32 => {
            let mut a = [0u8; 32];
            a.copy_from_slice(&pt);
            a
        }
        Ok(pt) => {
            log::warn!(
                "KeyRotation plaintext wrong length {} from {}",
                pt.len(),
                peer_ip
            );
            let deny = serde_json::json!({"type":"rotation_denied","reason":"invalid payload"});
            let _ = write_message_sync_on_stream(&mut stream, &deny.to_string()).await;
            return;
        }
        Err(_) => {
            log::warn!(
                "KeyRotation AEAD failed for '{}' — possible attack",
                machine_id
            );
            let deny =
                serde_json::json!({"type":"rotation_denied","reason":"authentication failed"});
            let _ = write_message_sync_on_stream(&mut stream, &deny.to_string()).await;
            return;
        }
    };

    // Authenticated — update stored sender pubkey using spawn_blocking to avoid guard across await
    let machine_id_clone = machine_id.clone();
    let rotation_result = tokio::task::spawn_blocking(move || {
        let state = shared_state.read().unwrap();
        let mut pm = state.pairing_manager.write().unwrap();
        pm.update_sender_pubkey(&machine_id, &new_pubkey_bytes)
    })
    .await;

    match rotation_result {
        Err(e) => {
            log::error!(
                "Failed to update sender pubkey for '{}': {}",
                machine_id_clone,
                e
            );
            let deny = serde_json::json!({"type":"rotation_denied","reason":"storage error"});
            let _ = write_message_sync_on_stream(&mut stream, &deny.to_string()).await;
            return;
        }
        Ok(Err(e)) => {
            log::error!(
                "Failed to update sender pubkey for '{}': {}",
                machine_id_clone,
                e
            );
            let deny = serde_json::json!({"type":"rotation_denied","reason":"storage error"});
            let _ = write_message_sync_on_stream(&mut stream, &deny.to_string()).await;
            return;
        }
        Ok(Ok(())) => {}
    }

    log::info!(
        "🔄 Key rotation accepted for machine '{}'",
        machine_id_clone
    );
    let ack = serde_json::json!({"type":"rotated"});
    let _ = write_message_sync_on_stream(&mut stream, &ack.to_string()).await;
    let _ = stream.flush().await;
}

/// Read a length-prefixed message: [4-byte big-endian length][JSON bytes].
async fn read_message(stream: &mut tokio::net::TcpStream) -> Result<String, std::io::Error> {
    use tokio::io::AsyncReadExt;
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 4096 {
        return Err(std::io::Error::other("message too large"));
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    String::from_utf8(buf).map_err(|e| std::io::Error::other(e.to_string()))
}

/// Write a length-prefixed message: [4-byte big-endian length][JSON bytes].
async fn write_message_sync_on_stream(
    stream: &mut tokio::net::TcpStream,
    json: &str,
) -> Result<(), std::io::Error> {
    use tokio::io::AsyncWriteExt;
    let bytes = json.as_bytes();
    let len = bytes.len() as u32;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(bytes).await?;
    Ok(())
}
