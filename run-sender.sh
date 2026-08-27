#!/bin/bash
# Run nmd-service in debug mode
# This script makes it easy to start the sender in foreground mode with debug logging

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="$PROJECT_ROOT/test-env"

# Check if test environment exists
if [ ! -f "$TEST_DIR/nmd-config.toml" ]; then
    echo "Error: Test environment not initialized. Run ./test-debug.sh first."
    exit 1
fi

echo "Starting nmd-service in debug mode..."
echo "Config: $TEST_DIR/nmd-config.toml"
echo "Press Ctrl-C to stop"
echo

# Run with debug logging
RUST_LOG=debug ./target/debug/nmd-service \
    --config "$TEST_DIR/nmd-config.toml" \
    --foreground
