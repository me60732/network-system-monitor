# Pairing System V1 Specification

## Overview

Network System Monitor implements a Trust-On-First-Use (TOFU) pairing system with per-machine ECDH-derived encryption keys.

### Security Model

- **Encryption**: ChaCha20-Poly1305 AEAD (confidentiality + authenticity in one operation)
- **Pairing**: TOFU — receiver detects unknown senders, shows pairing UI
- **Replay Protection**: Timestamp freshness (< 10s old) + monotonic sequence number tracking per machine_id session

### Key Types

| Key | Type | Size | Usage |
|-----|------|------|-------|
| Receiver Identity | Ed25519 | 64 bytes (32 priv + 32 pub) | Verify sender during pairing |
| Sender Identity | Ed25519 | 64 bytes (32 priv + 32 pub) | Identify sender during pairing |
| Shared Key | ChaCha20 | 32 bytes | Encrypt/decrypt packets via ECDH |

---

## Protocol Flow

```
┌─────────────────┐         ┌─────────────────┐
│   Receiver      │         │   Sender        │
│ (cosmic-applet) │         │ (nmd-service)   │
└────────┬────────┘         └────────┬────────┘
         │                            │
         │  1. UDP packet arrives     │
         │  (unknown sender)          │
         │  [pubkey][nonce][ciphertext]│
         │                            │
         ▼                            │
┌─────────────────┐                   │
│ TOFU Detection  │                   │
│ UI Prompt       │                   │
│ Accept/Deny     │                   │
└──────┬────────┘                     │
       │                              │
       │  2. Pairing Accepted         │
       │  Generate ECDH shared key    │
       │  Store in pairing.toml       │
       │                            ▼
       │                    ┌─────────────────┐
       │                    │ TCP Connection  │
       │                    │ Request pubkey  │
       │                    └──────┬────────┘
       │                           │
       │  3. Receiver sends        │
       │     X25519 public key     │
       │                           ▼
       │                  ┌─────────────────┐
       │                  │ Store pubkey    │
       │                  │ Derive ECDH     │
       │                  │ shared key      │
       │                  └──────┬────────┘
       │                         │
       │  4. Packets encrypted  │
       │     with ECDH key      │
       ▼                         ▼
┌─────────────────┐   ┌─────────────────┐
│  ChaCha20-Poly1305 AEAD  │
│  Decryption + Tag Verify  │
└─────────────────┘   └─────────────────┘
```

---

## Packet Wire Format

### ECDH-only (Phase 1)

```
[32-byte sender X25519 public key]
[12-byte nonce]
[ChaCha20-encrypted rkyv packet]
[16-byte Poly1305 tag]
```

Total: 32 + 12 + ciphertext_len + 16 bytes

### Encryption Flow (Sender)

```rust
// 1. Derive ECDH shared key from sender privkey + receiver pubkey
ecdh_key = x25519_dalek::StaticSecret::from(sender_privkey)
    .diffie_hellman(&receiver_pubkey)
    .as_bytes();

// 2. Build nonce (fixed prefix + counter)
nonce = build_nonce(b"NMDS", sequence_counter);

// 3. Serialize packet via rkyv
plaintext = rkyv::to_bytes(&packet)?;

// 4. Encrypt with ChaCha20-Poly1305
ciphertext = chacha20poly1305::seal_with_sender_pubkey(
    &cipher, &nonce, plaintext.as_ref(), &sender_pubkey
)?;
```

### Decryption Flow (Receiver)

```rust
// 1. Extract sender pubkey from first 32 bytes
sender_pubkey = packet[..32].try_into()?;

// 2. Extract nonce (next 12 bytes)
nonce = packet[32..44].try_into()?;

// 3. Extract ciphertext + tag
ciphertext_with_tag = &packet[44..];

// 4. Derive ECDH shared key from receiver privkey + sender pubkey
ecdh_key = x25519_dalek::StaticSecret::from(receiver_privkey)
    .diffie_hellman(&sender_pubkey)
    .as_bytes();

// 5. Decrypt and verify tag
plaintext = chacha20poly1305::open_with_sender_pubkey(
    &cipher, &nonce, ciphertext_with_tag, &sender_pubkey
)?;
```

---

## TOFU Pairing UI

When a UDP packet arrives from an unknown sender:

### Display Information

- **Machine ID**: From `packet.machine_id` field (first 20 bytes of machine name)
- **Sender IP**: From UDP socket's remote address
- **Accept/Deny Dropdown**: User chooses whether to pair

### Accept Flow

1. Generate receiver X25519 public key (if not already)
2. Derive ECDH shared key using sender's pubkey from packet
3. Store entry in `pairing.toml`:
   ```toml
   [[paired_machines]]
   machine_id = "pluto"
   shared_key = "<64-char hex>"
   paired_at = "2026-08-28T14:32:00Z"
   host = "192.168.1.100"
   ```
4. Send receiver's X25519 public key to sender via TCP (Phase 2)

### Deny Flow

1. Drop packet silently
2. No entry created in pairing.toml
3. Subsequent packets from same sender will trigger UI again

---

## Replay Protection

### Timestamp Freshness

- Each `MetricPacket` includes `timestamp: u64` (Unix seconds)
- Receiver accepts only packets where `now - timestamp < 10`
- Stale packets are rejected before decryption

### Sequence Tracking

- Per-machine sequence counter stored in `MachineConfig`
- First packet sets baseline sequence
- Subsequent packets must have `sequence > last_sequence`
- Monotonic increase enforced per machine_id session
- Replay of old sequence numbers rejected

---

## Configuration Files

### pairing.toml (Receiver)

```toml
# Machine pairing registry
# Generated automatically when accept pairing UI prompt

[[paired_machines]]
machine_id = "pluto"
shared_key = "a1b2c3d4e5f67890abcdef1234567890abcdef1234567890abcdef1234567890"  # 64 hex chars
paired_at = "2026-08-28T14:32:00Z"
host = "192.168.1.100"

[[paired_machines]]
machine_id = "server-alpha"
shared_key = "1a2b3c4d5e6f7890abcdef1234567890abcdef1234567890abcdef1234567890"
paired_at = "2026-08-27T09:15:00Z"
host = "192.168.1.50"
```

**Permissions**: `0600` (owner read/write only — contains shared encryption keys)

### Keypair Storage

#### Receiver Keypair (`~/.config/cosmic-applet/receiver.key`)
- Ed25519 identity keypair (64 bytes: 32 private + 32 public)
- Auto-generated on first applet start
- Used to verify sender ECDH key during pairing
- Permissions: `0600`

#### Sender Keypair (`~/.config/nmd/keypair.key`)
- Ed25519 identity keypair (auto-generated)
- Used for ECDH key derivation during pairing
- Permissions: `0600`

---

## Security Considerations

### Per-Machine Keys (Phase 1)

Each machine has its own ECDH-derived shared key:
- Compromising one machine's key doesn't affect others
- Key rotation possible via re-pairing
- No shared secret across all machines

### Man-in-the-Middle Protection

TOFU protects against first-connect MITM:
- First connection establishes trust baseline
- Subsequent connections must use same sender pubkey
- Different pubkey triggers pairing UI again

### Replay Attack Prevention

Two-layer protection:
1. Timestamp freshness (< 10s old)
2. Monotonic sequence numbers per session

Combined: Even if attacker captures packet, replaying it fails both checks.

---

## Future Enhancements (Phase 2+)

- [ ] Certificate-based authentication (X.509 + CA)
- [ ] Key rotation via secure channel
- [ ] Revocation lists for compromised keys
- [ ] Multi-factor pairing approval (PIN code)
- [ ] Automatic re-pairing when keys expire
