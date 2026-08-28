//! Network communication layer for receiving metric packets via UDP.
//!
//! Handles ChaCha20-Poly1305 AEAD decryption, sequence tracking, and packet deserialization.

pub mod udp_receiver;

// Shared packet-construction/encryption helpers for unit tests, integration tests, and benchmarks.
#[doc(hidden)]
pub mod test_support;

// UdpReceiver, UdpMessage, UdpPayload are defined in the udp_receiver submodule.
// Do not re-export them here to avoid import conflicts with main.rs types.