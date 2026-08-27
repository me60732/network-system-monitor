#!/bin/bash
# Run cosmic-applet with test configuration
# This script starts the applet configured to receive from the test nmd-service

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="$PROJECT_ROOT/test-env"

# Check if test environment exists
if [ ! -f "$TEST_DIR/applet-config.toml" ]; then
    echo "Error: Test environment not initialized. Run ./test-debug.sh first."
    exit 1
fi

# Create symlink in test-env so applet can find config.toml
if [ ! -L "$TEST_DIR/config.toml" ]; then
    ln -sf "$TEST_DIR/applet-config.toml" "$TEST_DIR/config.toml"
fi

echo "Starting cosmic-applet in debug mode..."
echo "Listening on: 127.0.0.1:51057"
echo "Secret key: $TEST_DIR/etc/nmd/secret.key"
echo "Config: $TEST_DIR/config.toml"
echo ""
echo "Watch for:"
echo "  - 'Received valid packet from 127.0.0.1' messages"
echo "  - CPU, memory, disk, network metrics updates"
echo ""
echo "Press Ctrl-C to stop"
echo ""

# Run with debug logging from test-env directory (so it finds config.toml)
# --test flag runs applet in standalone window instead of panel mode
cd "$TEST_DIR"
RUST_LOG=debug "$PROJECT_ROOT/target/debug/cosmic-applet" --test
