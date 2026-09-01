#!/bin/bash
# Run cosmic-applet receiver for 5-sender stress test
# This uses the test-env/5-senders/receiver-config.toml with machines a, b, c, d, e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_ROOT"

# Check if 5-sender test environment exists
if [ ! -f "test-env/5-senders/receiver-config.toml" ]; then
    echo "Error: 5-sender test environment not initialized."
    echo "Run: ./test-5-senders.sh setup"
    exit 1
fi

echo "Starting receiver for 5-sender stress test..."
echo "Listening on: 127.0.0.1:51057"
echo "Configured machines: a, b, c, d, e"
echo "Secret key: test-env/5-senders/secret.key"
echo ""
echo "Watch for metrics from all 5 machines"
echo "Press Ctrl-C to stop"
echo ""

# Build the receiver first
echo "Building cosmic-applet..."
cargo build --bin cosmic-applet

# Create symlink so applet can find config.toml
TEST_5_DIR="$PROJECT_ROOT/test-env/5-senders"
if [ ! -L "$TEST_5_DIR/config.toml" ]; then
    ln -sf "$TEST_5_DIR/receiver-config.toml" "$TEST_5_DIR/config.toml"
fi

# Run with --test flag for standalone window mode
cd "$TEST_5_DIR"
RUST_LOG=debug "$PROJECT_ROOT/target/debug/cosmic-applet" --test
