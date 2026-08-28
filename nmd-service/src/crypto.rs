//! # crypto — ChaCha20-Poly1305 wire encryption for MetricPacket transport (Pairing V1, Phase 2)
//!
//! Single owner of the encrypted wire format shared by `nmd-service` (sender) and
//! `cosmic-applet` (receiver). Both sides call into this module so the format can never drift.
//!
//! ## Wire Format (Phase 2, ECDH enabled)
//!
//! ```text
//! [32-byte sender X25519 public key][12-byte nonce][ChaCha20-encrypted rkyv packet][16-byte Poly1305 tag]
//! ```
//!
//! The sender's X25519 pubkey in the header lets the receiver:
//! 1. Identify which ECDH key to use for a paired sender
//! 2. Capture the real pubkey when creating a PairingRequest for an unknown sender
//!
//! The Poly1305 tag is appended to the ciphertext by the AEAD `encrypt()` call itself — it is
//! not a struct field. Decryption verifies the tag automatically; a tampered or forged packet
//! fails `decrypt()` and is dropped by the receiver.
//!
//! ## Nonce Discipline (CRITICAL — nonce reuse under one key breaks ChaCha20-Poly1305)
//!
//! Nonces are `[4-byte random per-sender prefix][8-byte big-endian counter]`. The counter
//! guarantees uniqueness within one sender session; the random prefix separates the nonce
//! spaces of multiple senders that share a key (which all Phase 1 senders do — see below).
//!
//! ## Phase 1 Key (TEMPORARY)
//!
//! All parties use the hardcoded [`TEMP_SHARED_KEY`]. Phase 2 replaces this with a per-machine
//! key derived via X25519 ECDH during pairing. Until then the wire is confidential only against
//! passive observers who don't read this source tree — acceptable for the stress-test phase.

use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, KeyInit},
};

/// TEMPORARY Phase 1 pre-shared key — replaced by an ECDH-derived per-machine key in Phase 2.
pub const TEMP_SHARED_KEY: [u8; 32] = [0x42; 32];

/// Length of the sender X25519 pubkey in the wire packet header.
pub const SENDER_PUBKEY_LEN: usize = 32;

/// Length of the plaintext nonce prefix on every wire packet.
pub const NONCE_LEN: usize = 12;

/// Length of the Poly1305 authentication tag appended to the ciphertext.
pub const TAG_LEN: usize = 16;

/// Total header size: sender pubkey + nonce.
pub const HEADER_SIZE: usize = SENDER_PUBKEY_LEN + NONCE_LEN;

/// Build a `ChaCha20Poly1305` cipher instance from a 32-byte key.
pub fn cipher_from_key(key: &[u8; 32]) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(key.into())
}

/// Assemble a 12-byte nonce from a per-sender random prefix and a monotonic counter.
///
/// The counter must never repeat within one sender session (use an `AtomicU64`); the prefix
/// must be random per sender instance so senders sharing [`TEMP_SHARED_KEY`] don't collide.
pub fn build_nonce(prefix: &[u8; 4], counter: u64) -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    nonce[..4].copy_from_slice(prefix);
    nonce[4..].copy_from_slice(&counter.to_be_bytes());
    nonce
}

/// Ed25519 → X25519 private key conversion (Dalek's canonical mapping).
///
/// The scalar is identical between the two curves; only the point representation differs.
pub fn ed25519_secret_to_x25519(secret: &[u8; 32]) -> x25519_dalek::StaticSecret {
    x25519_dalek::StaticSecret::from(*secret)
}

/// Derive X25519 public key from Ed25519 secret bytes.
pub fn derive_x25519_pubkey_from_ed25519_secret(secret: &[u8; 32]) -> [u8; 32] {
    let x25519_secret = ed25519_secret_to_x25519(secret);
    let pubkey = x25519_dalek::PublicKey::from(&x25519_secret);
    pubkey.to_bytes()
}

/// ECDH key derivation: shared secret from sender's X25519 secret and receiver's X25519 public key.
pub fn derive_ecdh_key(sender_secret: &[u8; 32], receiver_pubkey: &[u8; 32]) -> [u8; 32] {
    let sender_x25519_secret = ed25519_secret_to_x25519(sender_secret);
    let receiver_x25519_pubkey = x25519_dalek::PublicKey::from(*receiver_pubkey);
    let shared = sender_x25519_secret.diffie_hellman(&receiver_x25519_pubkey);
    *shared.as_bytes()
}

/// Encrypt a serialized packet into the full wire format: `[nonce][ciphertext+tag]`.
pub fn seal(
    cipher: &ChaCha20Poly1305,
    nonce_bytes: &[u8; NONCE_LEN],
    plaintext: &[u8],
) -> Result<Vec<u8>, std::io::Error> {
    let nonce = Nonce::from_slice(nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| std::io::Error::other(format!("ChaCha20-Poly1305 encryption failed: {e}")))?;

    let mut wire = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    wire.extend_from_slice(nonce_bytes);
    wire.extend_from_slice(&ciphertext);
    Ok(wire)
}

/// Encrypt with sender pubkey header: `[32-byte sender_pubkey][nonce][ciphertext+tag]`.
pub fn seal_with_sender_pubkey(
    cipher: &ChaCha20Poly1305,
    nonce_bytes: &[u8; NONCE_LEN],
    plaintext: &[u8],
    sender_x25519_pubkey: &[u8; 32],
) -> Result<Vec<u8>, std::io::Error> {
    let wire_without_header = seal(cipher, nonce_bytes, plaintext)?;

    let mut full_wire = Vec::with_capacity(HEADER_SIZE + plaintext.len() + TAG_LEN);
    full_wire.extend_from_slice(sender_x25519_pubkey);
    full_wire.extend_from_slice(&wire_without_header);
    Ok(full_wire)
}

/// Decrypt a full wire packet (`[nonce][ciphertext+tag]`) back into rkyv plaintext bytes.
///
/// AEAD tag verification is intrinsic to `decrypt()` — an `Err` here means the packet was
/// tampered with, encrypted under a different key, or truncated. The plaintext is returned in
/// an [`rkyv::util::AlignedVec`] so `rkyv::access` alignment requirements are always satisfied.
pub fn open(cipher: &ChaCha20Poly1305, wire: &[u8]) -> Result<rkyv::util::AlignedVec, String> {
    // Minimum viable packet: nonce + tag (empty plaintext). Anything shorter is garbage.
    if wire.len() < NONCE_LEN + TAG_LEN {
        return Err(format!("wire packet too short: {} bytes", wire.len()));
    }
    let (nonce_bytes, ciphertext) = wire.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "AEAD tag verification failed (tampered, forged, or wrong key)".to_string())?;

    // Copy into an aligned buffer for zero-copy rkyv access downstream.
    let mut aligned = rkyv::util::AlignedVec::with_capacity(plaintext.len());
    aligned.extend_from_slice(&plaintext);
    Ok(aligned)
}

/// Decrypt a wire packet with sender pubkey header, extracting both the pubkey and plaintext.
///
/// Returns: (sender_x25519_pubkey, decrypted_plaintext_bytes).
pub fn open_with_sender_pubkey(
    cipher: &ChaCha20Poly1305,
    wire: &[u8],
) -> Result<([u8; 32], rkyv::util::AlignedVec), String> {
    // Minimum viable packet: sender pubkey + nonce + tag
    if wire.len() < HEADER_SIZE + TAG_LEN {
        return Err(format!("wire packet too short: {} bytes", wire.len()));
    }
    let (sender_pubkey_bytes, remainder) = wire.split_at(SENDER_PUBKEY_LEN);
    let sender_pubkey: [u8; 32] = sender_pubkey_bytes
        .try_into()
        .map_err(|_| "sender pubkey slice has incorrect length".to_string())?;

    // Decrypt the remainder (nonce + ciphertext+tag)
    let plaintext = open(cipher, remainder)?;

    Ok((sender_pubkey, plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    /// Core Phase 1 guarantee: seal → open is lossless and self-authenticating.
    #[test]
    fn test_encryption_roundtrip() {
        let cipher = cipher_from_key(&TEMP_SHARED_KEY);
        let nonce = build_nonce(&[1, 2, 3, 4], 7);
        let plaintext = b"metric packet bytes";

        let wire = seal(&cipher, &nonce, plaintext).expect("seal should succeed");
        assert_eq!(wire.len(), NONCE_LEN + plaintext.len() + TAG_LEN);
        assert_eq!(&wire[..NONCE_LEN], &nonce);

        let recovered = open(&cipher, &wire).expect("open should succeed");
        assert_eq!(recovered.as_slice(), plaintext);
    }

    /// Test sender pubkey header: seal_with_sender_pubkey adds 32 bytes at start.
    #[test]
    fn test_seal_with_sender_pubkey() {
        let cipher = cipher_from_key(&TEMP_SHARED_KEY);
        let nonce = build_nonce(&[1, 2, 3, 4], 7);
        let plaintext = b"metric packet bytes";
        let sender_pubkey = [0xAB; 32];

        let wire = seal_with_sender_pubkey(&cipher, &nonce, plaintext, &sender_pubkey)
            .expect("seal_with_sender_pubkey should succeed");

        assert_eq!(wire.len(), HEADER_SIZE + plaintext.len() + TAG_LEN);
        assert_eq!(&wire[..SENDER_PUBKEY_LEN], &sender_pubkey);
        assert_eq!(&wire[SENDER_PUBKEY_LEN..][..NONCE_LEN], &nonce);
    }

    /// Test ECDH key derivation from Ed25519 secret.
    #[test]
    fn test_ed25519_to_x25519_conversion() {
        // Generate a random Ed25519 signing key
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let ed25519_secret = signing_key.to_bytes();

        // Convert to X25519 secret and derive pubkey
        let x25519_secret = ed25519_secret_to_x25519(&ed25519_secret);
        let x25519_pubkey_from_conversion =
            derive_x25519_pubkey_from_ed25519_secret(&ed25519_secret);

        // Compare with direct X25519 pubkey derivation
        let direct_pubkey = x25519_dalek::PublicKey::from(&x25519_secret);
        assert_eq!(x25519_pubkey_from_conversion, direct_pubkey.to_bytes());
    }

    /// Test ECDH shared secret derivation.
    #[test]
    fn test_ecdh_key_derivation() {
        // Generate two Ed25519 keypairs
        let alice_signing = SigningKey::generate(&mut rand::rngs::OsRng);
        let bob_signing = SigningKey::generate(&mut rand::rngs::OsRng);

        let alice_secret = alice_signing.to_bytes();
        let bob_secret = bob_signing.to_bytes();

        // Derive X25519 pubkeys
        let alice_pubkey = derive_x25519_pubkey_from_ed25519_secret(&alice_secret);
        let bob_pubkey = derive_x25519_pubkey_from_ed25519_secret(&bob_secret);

        // Each side derives the shared secret
        let alice_shared = derive_ecdh_key(&alice_secret, &bob_pubkey);
        let bob_shared = derive_ecdh_key(&bob_secret, &alice_pubkey);

        // Both must arrive at the same shared secret
        assert_eq!(alice_shared, bob_shared);
    }

    /// Flipping any ciphertext bit must fail AEAD tag verification.
    #[test]
    fn test_tamper_detection() {
        let cipher = cipher_from_key(&TEMP_SHARED_KEY);
        let nonce = build_nonce(&[0; 4], 1);
        let wire = seal(&cipher, &nonce, b"payload").expect("seal");

        // Tamper with ciphertext body, tag, and nonce — all must be rejected.
        for idx in [NONCE_LEN, wire.len() - 1, 0] {
            let mut mutated = wire.clone();
            mutated[idx] ^= 0x01;
            assert!(
                open(&cipher, &mutated).is_err(),
                "bit flip at {idx} must fail"
            );
        }
    }

    /// A packet encrypted under a different key must not decrypt.
    #[test]
    fn test_wrong_key_rejected() {
        let cipher = cipher_from_key(&TEMP_SHARED_KEY);
        let wrong = cipher_from_key(&[0x99; 32]);
        let wire = seal(&cipher, &build_nonce(&[0; 4], 1), b"payload").expect("seal");
        assert!(open(&wrong, &wire).is_err());
    }

    /// Truncated packets (shorter than nonce + tag) are rejected without panicking.
    #[test]
    fn test_short_packet_rejected() {
        let cipher = cipher_from_key(&TEMP_SHARED_KEY);
        assert!(open(&cipher, &[]).is_err());
        assert!(open(&cipher, &[0u8; NONCE_LEN + TAG_LEN - 1]).is_err());
    }

    /// Nonce layout: prefix in the first 4 bytes, big-endian counter in the last 8.
    #[test]
    fn test_nonce_construction() {
        let nonce = build_nonce(&[0xAA, 0xBB, 0xCC, 0xDD], 0x0102_0304_0506_0708);
        assert_eq!(&nonce[..4], &[0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(
            &nonce[4..],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
    }
}
