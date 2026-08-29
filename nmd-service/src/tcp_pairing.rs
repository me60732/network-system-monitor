//! TCP pairing client — connects to the receiver for initial pairing or key rotation.
//!
//! Initial pairing: sender has no receiver_pubkey yet → sends PairingHello → waits for
//! PairingAccept (up to 120s) → saves receiver_pubkey to config.
//!
//! Key rotation: sender's keypair is > 24h old → generates new keypair → sends
//! KeyRotation authenticated with old ECDH key → receiver auto-accepts → sender saves
//! new keypair to disk.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Result of a pairing or rotation attempt.
#[derive(Debug)]
pub enum PairingResult {
    /// Successfully paired — contains the receiver's hex-encoded X25519 pubkey.
    Accepted(String),
    /// Receiver denied the request.
    Denied,
    /// Connection failed or timed out.
    Failed(String),
}

/// Attempt initial pairing with the receiver.
///
/// Connects to `host:port`, sends PairingHello with our sender pubkey,
/// waits up to 120s for the user to accept/deny in the receiver UI.
pub fn request_pairing(
    host: &str,
    port: u16,
    machine_id: &str,
    sender_x25519_pubkey: &[u8; 32],
) -> PairingResult {
    let addr = format!("{}:{}", host, port);
    log::info!("🔌 Connecting to receiver at {} for pairing...", addr);

    let mut stream = match TcpStream::connect_timeout(
        &addr
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:51057".parse().unwrap()),
        Duration::from_secs(10),
    ) {
        Ok(s) => s,
        Err(e) => return PairingResult::Failed(format!("TCP connect failed: {}", e)),
    };

    stream.set_read_timeout(Some(Duration::from_secs(120))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(10))).ok();

    let hello = serde_json::json!({
        "type": "hello",
        "machine_id": machine_id,
        "sender_pubkey": hex::encode(sender_x25519_pubkey),
    });

    if let Err(e) = write_message(&mut stream, &hello.to_string()) {
        return PairingResult::Failed(format!("Failed to send PairingHello: {}", e));
    }

    log::info!("⏳ Waiting for user to accept/deny pairing in the receiver UI (up to 120s)...");

    match read_message(&mut stream) {
        Ok(msg) => match serde_json::from_str::<serde_json::Value>(&msg) {
            Ok(v) => match v.get("type").and_then(|t| t.as_str()) {
                Some("accept") => {
                    let pubkey = v
                        .get("receiver_pubkey")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .to_string();
                    if pubkey.len() != 64 {
                        return PairingResult::Failed(
                            "Invalid receiver_pubkey length in accept".into(),
                        );
                    }
                    log::info!("✅ Pairing accepted by receiver");
                    PairingResult::Accepted(pubkey)
                }
                Some("deny") => {
                    log::info!("❌ Pairing denied by receiver");
                    PairingResult::Denied
                }
                other => PairingResult::Failed(format!("Unexpected response type: {:?}", other)),
            },
            Err(e) => PairingResult::Failed(format!("JSON parse error: {}", e)),
        },
        Err(e) => PairingResult::Failed(format!("Failed to read response: {}", e)),
    }
}

/// Attempt an authenticated key rotation.
///
/// Uses the old ECDH key (derived from old_ed25519_secret + receiver_pubkey) to
/// encrypt the new sender pubkey. The receiver verifies with the stored old pubkey
/// and auto-accepts if authenticated.
pub fn request_key_rotation(
    host: &str,
    port: u16,
    machine_id: &str,
    old_ed25519_secret: &[u8; 32],
    receiver_pubkey_hex: &str,
    new_sender_x25519_pubkey: &[u8; 32],
) -> PairingResult {
    use chacha20poly1305::{
        ChaCha20Poly1305, Nonce,
        aead::{Aead, KeyInit},
    };

    let addr = format!("{}:{}", host, port);
    log::info!("🔄 Connecting to receiver at {} for key rotation...", addr);

    let mut stream = match TcpStream::connect_timeout(
        &addr
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:51057".parse().unwrap()),
        Duration::from_secs(10),
    ) {
        Ok(s) => s,
        Err(e) => return PairingResult::Failed(format!("TCP connect failed: {}", e)),
    };

    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(10))).ok();

    // Decode receiver pubkey
    let receiver_pubkey_bytes = match hex::decode(receiver_pubkey_hex) {
        Ok(b) if b.len() == 32 => {
            let mut a = [0u8; 32];
            a.copy_from_slice(&b);
            a
        }
        _ => return PairingResult::Failed("Invalid receiver_pubkey hex".into()),
    };

    // Derive old ECDH key
    let old_ecdh_key = crate::crypto::derive_ecdh_key(old_ed25519_secret, &receiver_pubkey_bytes);

    // Encrypt new sender pubkey with old ECDH key
    // Generate random 12-byte nonce
    let mut nonce_bytes = [0u8; 12];
    use std::fs::File;
    use std::io::Read as IoRead;
    if let Ok(mut f) = File::open("/dev/urandom") {
        f.read_exact(&mut nonce_bytes).ok();
    }

    let cipher = ChaCha20Poly1305::new((&old_ecdh_key).into());
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = match cipher.encrypt(nonce, new_sender_x25519_pubkey.as_ref()) {
        Ok(ct) => ct,
        Err(e) => return PairingResult::Failed(format!("Encryption failed: {}", e)),
    };

    let rotate_msg = serde_json::json!({
        "type": "rotate",
        "machine_id": machine_id,
        "nonce": hex::encode(&nonce_bytes),
        "ciphertext": hex::encode(&ciphertext),
    });

    if let Err(e) = write_message(&mut stream, &rotate_msg.to_string()) {
        return PairingResult::Failed(format!("Failed to send KeyRotation: {}", e));
    }

    match read_message(&mut stream) {
        Ok(msg) => match serde_json::from_str::<serde_json::Value>(&msg) {
            Ok(v) => match v.get("type").and_then(|t| t.as_str()) {
                Some("rotated") => {
                    log::info!("✅ Key rotation accepted by receiver");
                    PairingResult::Accepted(String::new()) // pubkey not needed for rotation
                }
                Some("rotation_denied") => {
                    let reason = v
                        .get("reason")
                        .and_then(|r| r.as_str())
                        .unwrap_or("unknown");
                    log::warn!("❌ Key rotation denied: {}", reason);
                    PairingResult::Denied
                }
                other => {
                    PairingResult::Failed(format!("Unexpected rotation response: {:?}", other))
                }
            },
            Err(e) => PairingResult::Failed(format!("JSON parse error: {}", e)),
        },
        Err(e) => PairingResult::Failed(format!("Failed to read response: {}", e)),
    }
}

/// Read a length-prefixed TCP message.
fn read_message(stream: &mut TcpStream) -> Result<String, std::io::Error> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 4096 {
        return Err(std::io::Error::other("message too large"));
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| std::io::Error::other(e.to_string()))
}

/// Write a length-prefixed TCP message.
fn write_message(stream: &mut TcpStream, json: &str) -> Result<(), std::io::Error> {
    let bytes = json.as_bytes();
    let len = bytes.len() as u32;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(bytes)?;
    stream.flush()
}
