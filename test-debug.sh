#!/bin/bash
# Test simulation script for network-system-monitor
# Runs nmd-service and cosmic-applet locally with debug logging

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="$PROJECT_ROOT/test-env"

echo "=== Network System Monitor Debug Test ==="
echo

# 1. Create test environment directory
echo "[1/6] Setting up test environment..."
mkdir -p "$TEST_DIR/etc/nmd"

# 2. Generate test secret key (32 bytes for HMAC-SHA256)
echo "[2/6] Generating test secret key..."
if [ ! -f "$TEST_DIR/etc/nmd/secret.key" ]; then
    # Generate 32 hex bytes (64 chars) for the secret key
    # Use printf to avoid trailing newline that echo adds
    printf "0123456789abcdef0123456789abcdef" > "$TEST_DIR/etc/nmd/secret.key"
    chmod 600 "$TEST_DIR/etc/nmd/secret.key"
    echo "  ✓ Created secret key at $TEST_DIR/etc/nmd/secret.key"
else
    echo "  ✓ Using existing secret key"
fi

# 3. Create test configuration for nmd-service
echo "[3/6] Creating test configuration..."
cat > "$TEST_DIR/nmd-config.toml" <<EOF
# Test configuration for nmd-service
host = "127.0.0.1"
port = 51057
interval_ms = 2000
machine_id = "test-machine"
hmac_secret_path = "$TEST_DIR/etc/nmd/secret.key"
EOF
echo "  ✓ Created $TEST_DIR/nmd-config.toml"

# 4. Create test configuration for cosmic-applet receiver
cat > "$TEST_DIR/applet-config.toml" <<EOF
udp_port = 51057
hmac_secret_path = "$TEST_DIR/etc/nmd/secret.key"
auto_expand_grid = true

[[machines]]
name = "test-machine"
host = "127.0.0.1"
port = 51057
enabled = true
show_cpu = true
show_memory = true
show_disk = true
show_network = true
show_uptime = true
show_gpu_vram = true
show_temperature = true
EOF
echo "  ✓ Created $TEST_DIR/applet-config.toml"

# 5. Build both binaries
echo "[4/6] Building binaries..."
echo "  Building nmd-service..."
cargo build --package nmd-service 2>&1 | grep -E "(Compiling|Finished)" || true
echo "  Building cosmic-applet..."
cargo build --package cosmic-applet 2>&1 | grep -E "(Compiling|Finished)" || true
echo "  ✓ Build complete"

# 6. Display instructions
echo
echo "[5/6] Test environment ready!"
echo
echo "=== Running in Debug Mode ==="
echo
echo "To start the test simulation, open TWO terminal windows:"
echo
echo "Terminal 1 (nmd-service sender):"
echo "  RUST_LOG=debug ./target/debug/nmd-service --config $TEST_DIR/nmd-config.toml --foreground"
echo
echo "Terminal 2 (cosmic-applet receiver - if standalone test):"
echo "  RUST_LOG=debug ./target/debug/cosmic-applet"
echo
echo "Or just run the cosmic-applet from COSMIC panel if already installed."
echo
echo "The applet should automatically receive metrics from the local nmd-service."
echo "Watch for debug logs showing:"
echo "  - nmd-service: 'Sent metrics — seq=N, cpu=X%, mem=Y%'"
echo "  - cosmic-applet UDP receiver: 'Received valid packet from 127.0.0.1'"
echo
echo "[6/6] Press Ctrl-C in each terminal to stop"
echo
echo "=== Quick Start (Single Command) ==="
echo "Run nmd-service in foreground:"
echo "  RUST_LOG=debug ./target/debug/nmd-service --config $TEST_DIR/nmd-config.toml --foreground"
