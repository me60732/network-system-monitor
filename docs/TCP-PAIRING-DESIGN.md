# TCP Pairing Design

## Overview

TCP pairing is the Phase 2 extension to the TOFU pairing system. It enables automatic sender setup after receiver acceptance, eliminating manual configuration of `receiver_pubkey` in the nmd-service config file.

### Why TCP?

UDP is push-based (senders push to receiver), but we need a pull-based channel for:
1. Receiver sending its X25519 public key to sender
2. Sender receiving and storing this pubkey for ECDH key derivation

TCP provides reliable, ordered delivery with connection handshake — perfect for this setup phase.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Desktop Machine                              │
│  ┌──────────────────────┐    ┌──────────────────────────────┐   │
│  │  cosmic-applet       │    │  TCP Listener (nmd-service)  │   │
│  │  - UdpReceiver       │    │  - Listens on port 51058     │   │
│  │  - PairingManager    │    │  - Accepts connections       │   │
│  │  - Panel UI          │    │  - Sends receiver pubkey     │   │
│  └──────────┬───────────┘    └───────────────▲──────────────┘   │
│             │                                 │                  │
│             │ UDP packets (push)              │ TCP connection  │
│             │ ChaCha20-Poly1305               │ (pull, setup)   │
│             ▼                                 │                  │
└──────────────────────────────────────────────┼──────────────────┘
                                               │
                                               │ TCP handshake + pubkey transfer
                                               │ Port: 51058 (configurable)
                                               │
┌──────────────────────────────────────────────┼──────────────────┐
│                   Remote Machine              │                  │
│  ┌──────────────────────┐    ┌──────────────────────────────┐   │
│  │  nmd-service         │    │  TCP Client (cosmic-applet)  │   │
│  │  - UdpSender         │    │  - Connects to desktop:51058 │   │
│  │  - ConfigManager     │    │  - Receives receiver pubkey  │   │
│  │  - Systemd service   │    │  - Stores in config.toml     │   │
│  └──────────┬───────────┘    └───────────────▲──────────────┘   │
│             │                                 │                  │
│             │ UDP packets (push)              │ TCP connection  │
│             │ ChaCha20-Poly1305               │ (pull, setup)   │
│             ▼                                 │                  │
└──────────────────────────────────────────────┴──────────────────┘
```

---

## Protocol Message Format

### Request (Sender → Receiver)

```json
{
  "type": "pubkey_request",
  "machine_id": "pluto",      // Sender's machine ID
  "sender_pubkey_hex": "..."  // Sender's X25519 public key (64 hex chars)
}
```

### Response (Receiver → Sender)

```json
{
  "type": "pubkey_response",
  "receiver_pubkey_hex": "..."  // Receiver's X25519 public key (64 hex chars)
}
```

---

## Flow Details

### Step 1: Initial UDP Packet (TOFU Detection)

```
Sender → Receiver:
[sender_x25519_pubkey][nonce][encrypted_packet]

Receiver detects unknown sender → Shows pairing UI
```

### Step 2: User Accepts Pairing

```
PairingManager.add_pairing(
    machine_id = "pluto",
    sender_pubkey = <from packet>,
    host = "192.168.1.100"
)

Generate ECDH shared key using:
- Sender's pubkey (from packet)
- Receiver's Ed25519 privkey → converted to X25519

Store in pairing.toml
```

### Step 3: TCP Connection Initiated by Sender

```
Sender (nmd-service) connects to Receiver (cosmic-applet):
TCP socket → desktop:51058

Send request:
{
  "type": "pubkey_request",
  "machine_id": "pluto",
  "sender_pubkey_hex": "<sender's X25519 pubkey hex>"
}
```

### Step 4: Receiver Validates & Responds

```
Receiver validates:
- machine_id matches paired entry
- sender_pubkey matches paired entry

If valid → respond with receiver's X25519 pubkey:

{
  "type": "pubkey_response",
  "receiver_pubkey_hex": "<receiver's X25519 pubkey hex>"
}

Close TCP connection (one-time exchange)
```

### Step 5: Sender Stores & Derives Shared Key

```
Sender receives response:
receiver_pubkey = <from JSON>

Store in config.toml:
receiver_pubkey = "<64-char hex>"

Derive ECDH shared key:
ecdh_key = sender_privkey.diffie_hellman(&receiver_pubkey)

Subsequent packets encrypted with this key
```

---

## Security Considerations

### Authentication

TCP pairing uses the same TOFU trust model:

1. **Initial UDP packet**: Establishes machine_id + sender_pubkey baseline
2. **TCP request**: Includes same machine_id + sender_pubkey for verification
3. **Receiver validates**: Matches against pairing.toml entry

If attacker intercepts TCP connection:
- Cannot forge receiver pubkey (only legitimate receiver has privkey)
- Sender would reject wrong pubkey during ECDH key derivation

### Confidentiality

TCP connection is local network only:
- No encryption needed (UDP packets already encrypted with ChaCha20-Poly1305)
- One-time exchange (pubkey sent, connection closed)

### Replay Protection

TCP pairing is one-shot:
- Each sender connects once during first-pairing setup
- No ongoing TCP session needed
- Re-pairing requires new UDP packet → new TOFU detection

---

## Error Handling

### Invalid machine_id or sender_pubkey

```
Receiver response:
{
  "type": "error",
  "code": "unpaired_machine",
  "message": "Machine ID 'pluto' not in pairing registry"
}

TCP connection closed immediately
```

Sender behavior:
- Log error to debug output
- Continue sending UDP packets (TOFU UI will show again)
- No config update (receiver_pubkey remains unset)

### Connection Timeout

Receiver timeout: 5 seconds per TCP connection

If no request received:
- Close connection silently
- No error logged (normal cleanup)

### Invalid JSON

Both sides validate message format:

```rust
// Receiver validates request
match serde_json::from_str::<PubkeyRequest>(&buffer) {
    Ok(req) => {
        if req.type != "pubkey_request" { /* reject */ }
        // Validate machine_id + sender_pubkey
    }
    Err(e) => {
        log::warn!("Invalid TCP pairing request: {}", e);
        close_connection();
    }
}
```

---

## Configuration

### Port Configuration (Optional)

Default port: `51058` (distinct from UDP port `51057`)

#### Receiver (cosmic-applet config.toml)

```toml
tcp_pairing_port = 51058  # Optional, default is 51058
```

#### Sender (nmd-service config.toml)

No configuration needed — sender connects to receiver's UDP destination IP + TCP port.

---

## Implementation Notes

### Receiver Side (cosmic-applet/src/tcp_pairing.rs)

```rust
pub struct TcpPairingListener {
    socket: std::net::TcpListener,
}

impl TcpPairingListener {
    pub fn new(port: u16) -> Result<Self, io::Error> {
        let socket = std::net::TcpListener::bind(("0.0.0.0", port))?;
        Ok(TcpPairingListener { socket })
    }

    pub fn accept(&self, pairing_manager: &RwLock<PairingManager>) {
        while let Ok((stream, addr)) = self.socket.accept() {
            handle_tcp_pairing(stream, addr, pairing_manager);
        }
    }
}
```

### Sender Side (nmd-service/src/tcp_pairing.rs)

```rust
pub fn fetch_receiver_pubkey(
    host: &str,
    port: u16,
    machine_id: &str,
    sender_pubkey_hex: &str,
) -> Result<String, io::Error> {
    let mut stream = std::net::TcpStream::connect((host, port))?;
    
    // Send request
    let request = serde_json::json!({
        "type": "pubkey_request",
        "machine_id": machine_id,
        "sender_pubkey_hex": sender_pubkey_hex
    });
    serde_json::to_writer(&mut stream, &request)?;
    
    // Receive response
    let response: PubkeyResponse = serde_json::from_reader(stream)?;
    
    Ok(response.receiver_pubkey_hex)
}
```

---

## Future Enhancements

- [ ] Multiple TCP ports for load balancing (Phase 3)
- [ ] TLS encryption for TCP channel (if cross-network pairing needed)
- [ ] Automatic re-pairing when keys expire
- [ ] Multi-factor approval (PIN code over TCP)
