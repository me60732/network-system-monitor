//! # crypto — ChaCha20-Poly1305 wire encryption for MetricPacket transport (Pairing V1, Phase 1)
//!
//! Single owner of the encrypted wire format shared by `nmd-service` (sender) and
//! `cosmic-applet` (receiver). Both sides call into this module so the format can never drift.
//!
//! ## Wire Format
//!
//! ```text
//! [12-byte nonce][ChaCha20-encrypted rkyv-serialized MetricPacket][16-byte Poly1305 tag]
//! ```
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
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};

/// TEMPORARY Phase 1 pre-shared key — replaced by an ECDH-derived per-machine key in Phase 2.
pub const TEMP_SHARED_KEY: [u8; 32] = [0x42; 32];

/// Length of the plaintext nonce prefix on every wire packet.
pub const NONCE_LEN: usize = 12;

/// Length of the Poly1305 authentication tag appended to the ciphertext.
pub const TAG_LEN: usize = 16;

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

/// Decrypt a full wire packet (`[nonce][ciphertext+tag]`) back into rkyv plaintext bytes.
///
/// AEAD tag verification is intrinsic to `decrypt()` — an `Err` here means the packet was
/// tampered with, encrypted under a different key, or truncated. The plaintext is returned in
/// an [`rkyv::util::AlignedVec`] so `rkyv::access` alignment requirements are always satisfied.
pub fn open(
    cipher: &ChaCha20Poly1305,
    wire: &[u8],
) -> Result<rkyv::util::AlignedVec, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(open(&cipher, &mutated).is_err(), "bit flip at {idx} must fail");
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
        assert_eq!(&nonce[4..], &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }
}
