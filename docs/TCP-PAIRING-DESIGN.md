# TCP Pairing Handshake — Future Design

## Motivation

The current pairing workflow requires manual steps:
1. Open the receiver applet, go to Settings, copy the Receiver Public Key (hex)
2. Paste it into the sender's `/etc/nmd/config.toml` as `receiver_pubkey = "..."`
3. Restart nmd-service
4. Accept the pairing request in the applet UI

A TCP-based pairing handshake would eliminate steps 1–3, enabling zero-touch setup.

## Proposed Design

### Overview

```
Sender first start (no receiver_pubkey configured):
  TCP connect to receiver:51057 → sends PairingHello { machine_id, sender_x25519_pubkey }
  Receiver UI shows: "Machine 'pluto' (192.168.1.x) wants to connect — Accept / Deny"
  User accepts → receiver sends PairingAccept { receiver_x25519_pubkey } over TCP
  Sender saves receiver_pubkey to /etc/nmd/config.toml, closes TCP connection
  Sender derives ECDH key, begins UDP metrics stream

All subsequent starts (receiver_pubkey already configured):
  Skip TCP entirely — goes straight to UDP with ECDH key
```

### Port Usage

TCP and UDP are independent protocols at the OS level. Both can use port 51057 simultaneously:
- `51057/TCP` — pairing handshake (connect, exchange pubkeys, disconnect)
- `51057/UDP` — ongoing metrics stream (always active)

The receiver binds both in parallel at startup.

### Wire Protocol (TCP)

All messages are length-prefixed JSON for simplicity.

**PairingHello** (sender → receiver):
```json
{ "type": "hello", "machine_id": "pluto", "sender_pubkey": "hex-encoded-32-bytes" }
```

**PairingAccept** (receiver → sender, after user approval):
```json
{ "type": "accept", "receiver_pubkey": "hex-encoded-32-bytes" }
```

**PairingDeny** (receiver → sender, after user denial):
```json
{ "type": "deny" }
```

### Sender Behaviour

- On startup, if `receiver_pubkey` is absent from config:
  - Attempt TCP connect to `host:port` with 5s timeout
  - Send PairingHello
  - Wait up to 120s for PairingAccept or PairingDeny
  - On Accept: write `receiver_pubkey` to config file, derive ECDH key, start UDP
  - On Deny or timeout: log error, exit (systemd will retry after RestartSec)
- If `receiver_pubkey` is present: skip TCP, go straight to UDP

### Receiver Behaviour

- TCP listener runs in a separate tokio task alongside the UDP listener
- On incoming TCP connection: read PairingHello, push to `pending_pairings` (same queue as today)
- When user accepts in UI: send PairingAccept over the held TCP connection, close it
- When user denies: send PairingDeny, close connection

### Implementation Notes

- The TCP connection should be held open while awaiting user approval (up to 120s)
- Use `tokio::net::TcpListener` alongside existing `tokio::net::UdpSocket`
- The pairing queue and UI are unchanged — TCP just provides a new path into `pending_pairings`
- After pairing, all communication remains UDP — TCP is only used once per machine per installation

### Security Notes

- The TCP PairingHello is unauthenticated — anyone on the LAN can send one
- This is acceptable because the user must explicitly approve each request in the UI
- The `receiver_pubkey` sent in PairingAccept is not secret (it's a public key)
- The ECDH-derived shared key is never transmitted — each side derives it independently

## Status

Not yet implemented. Tracked here for future reference.
Prerequisite: ECDH-only UDP path (completed — TEMP_SHARED_KEY removed).
