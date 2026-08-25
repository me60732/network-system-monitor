//! Network communication layer for receiving metric packets via UDP.
//!
//! Handles HMAC verification, sequence tracking, and packet deserialization.

pub mod udp_receiver;

// UdpReceiver, UdpMessage, UdpPayload are defined in the udp_receiver submodule.
// Do not re-export them here to avoid import conflicts with main.rs types.