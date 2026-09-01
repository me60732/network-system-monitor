#!/bin/bash
# Test script: 5 concurrent UDP senders → 1 receiver
# Validates performance monitoring (Item 7.2) and packet loss detection (Item 7.3)
# All senders run on localhost, simulating 5 remote machines

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

TEST_DIR="test-env/5-senders"
SECRET_KEY="$TEST_DIR/secret.key"
RECEIVER_LOG="$TEST_DIR/receiver.log"
RECEIVER_PORT=51057
REFRESH_INTERVAL=1

# Machine names
MACHINES=("a" "b" "c" "d" "e")

# Cleanup function
cleanup() {
    echo -e "${YELLOW}Cleaning up processes...${NC}"
    pkill -f "nmd-service.*test-env/5-senders" || true
    pkill -f "cosmic-applet.*test-env/5-senders" || true
    echo -e "${GREEN}Cleanup complete${NC}"
}

# Setup test environment
setup() {
    echo -e "${BLUE}=== Setting up 5-sender test environment ===${NC}"
    
    # Create test directory
    rm -rf "$TEST_DIR"
    mkdir -p "$TEST_DIR/logs"
    
    # Generate shared secret key (32 random bytes) for ChaCha20-Poly1305 encryption
    echo -e "${BLUE}Generating shared encryption key...${NC}"
    head -c 32 /dev/urandom > "$SECRET_KEY"
    chmod 600 "$SECRET_KEY"
    echo -e "${GREEN}✓ Secret key generated: $SECRET_KEY${NC}"
    
    # Create config for each sender
    for machine in "${MACHINES[@]}"; do
        cat > "$TEST_DIR/config-$machine.toml" <<EOF
# nmd-service config for test machine '$machine'
host = "127.0.0.1"
port = $RECEIVER_PORT
refresh_interval_secs = $REFRESH_INTERVAL
machine_id = "$machine"
EOF
        echo -e "${GREEN}✓ Created config for machine '$machine'${NC}"
    done
    
    # Create receiver config (cosmic-applet config)
    cat > "$TEST_DIR/receiver-config.toml" <<EOF
# Receiver config for test
udp_port = $RECEIVER_PORT

[[machines]]
name = "a"
host = "127.0.0.1"
port = $RECEIVER_PORT
enabled = true

[[machines]]
name = "b"
host = "127.0.0.1"
port = $RECEIVER_PORT
enabled = true

[[machines]]
name = "c"
host = "127.0.0.1"
port = $RECEIVER_PORT
enabled = true

[[machines]]
name = "d"
host = "127.0.0.1"
port = $RECEIVER_PORT
enabled = true

[[machines]]
name = "e"
host = "127.0.0.1"
port = $RECEIVER_PORT
enabled = true
EOF
    echo -e "${GREEN}✓ Created receiver config${NC}"
    
    echo ""
    echo -e "${GREEN}=== Setup complete ===${NC}"
}

# Start all senders
start_senders() {
    echo ""
    echo -e "${BLUE}=== Starting 5 sender processes ===${NC}"
    
    for machine in "${MACHINES[@]}"; do
        LOG_FILE="$TEST_DIR/logs/sender-$machine.log"
        CONFIG="$TEST_DIR/config-$machine.toml"
        
        # Start sender in background with RUST_LOG=debug for performance monitoring
        RUST_LOG=debug cargo run --bin nmd-service --release -- \
            --config "$CONFIG" \
            --foreground \
            > "$LOG_FILE" 2>&1 &
        
        SENDER_PID=$!
        echo -e "${GREEN}✓ Started sender '$machine' (PID $SENDER_PID) → $LOG_FILE${NC}"
        
        # Brief delay to stagger startups
        sleep 0.2
    done
    
    echo ""
    echo -e "${GREEN}All 5 senders running!${NC}"
}

# Display receiver instructions
show_receiver_instructions() {
    echo ""
    echo -e "${BLUE}=== Receiver Instructions ===${NC}"
    echo ""
    echo -e "${YELLOW}In a separate terminal, run:${NC}"
    echo ""
    echo -e "  ${GREEN}RUST_LOG=debug cargo run --bin cosmic-applet -- \\${NC}"
    echo -e "  ${GREEN}  $TEST_DIR/receiver-config.toml${NC}"
    echo ""
    echo -e "${YELLOW}Or start the receiver automatically:${NC}"
    echo ""
    echo -e "  ${GREEN}./test-5-senders.sh receiver${NC}"
    echo ""
}

# Start receiver (if requested)
start_receiver() {
    echo ""
    echo -e "${BLUE}=== Starting receiver ===${NC}"
    
    RUST_LOG=debug cargo run --bin cosmic-applet --release -- \
        "$TEST_DIR/receiver-config.toml" \
        > "$RECEIVER_LOG" 2>&1 &
    
    RECEIVER_PID=$!
    echo -e "${GREEN}✓ Started receiver (PID $RECEIVER_PID) → $RECEIVER_LOG${NC}"
    echo ""
    echo -e "${YELLOW}Receiver listening on port $RECEIVER_PORT${NC}"
}

# Monitor logs in real-time
monitor_logs() {
    echo ""
    echo -e "${BLUE}=== Monitoring logs (Ctrl+C to stop) ===${NC}"
    echo ""
    echo -e "${YELLOW}Performance warnings (>50ms collectors):${NC}"
    echo ""
    
    # Monitor all sender logs for performance warnings
    tail -f "$TEST_DIR"/logs/sender-*.log 2>/dev/null | grep --line-buffered -E "WARN|took.*ms" &
    TAIL_PID=$!
    
    # Monitor receiver log if it exists
    if [ -f "$RECEIVER_LOG" ]; then
        echo ""
        echo -e "${YELLOW}Receiver events:${NC}"
        tail -f "$RECEIVER_LOG" 2>/dev/null | grep --line-buffered -E "New session|Packet loss|WARN" &
        TAIL_RX_PID=$!
    fi
    
    # Wait for user interrupt
    trap "kill $TAIL_PID $TAIL_RX_PID 2>/dev/null; exit 0" INT
    wait
}

# Show performance summary
show_summary() {
    echo ""
    echo -e "${BLUE}=== Performance Summary ===${NC}"
    echo ""
    
    for machine in "${MACHINES[@]}"; do
        LOG_FILE="$TEST_DIR/logs/sender-$machine.log"
        if [ -f "$LOG_FILE" ]; then
            echo -e "${GREEN}Machine '$machine':${NC}"
            
            # Count total aggregations (look for the debug message)
            TOTAL=$(grep -c "Metrics aggregation completed\|Total metrics aggregation took" "$LOG_FILE" 2>/dev/null || echo "0")
            # Divide by 2 since we get both messages per aggregation
            TOTAL=$((TOTAL / 2))
            if [ "$TOTAL" -eq 0 ]; then
                # Fallback: count "Sent metrics" messages
                TOTAL=$(grep -c "Sent metrics" "$LOG_FILE" 2>/dev/null || echo "0")
            fi
            echo "  Total aggregations: $TOTAL"
            
            # Count slow aggregations (>50ms)
            SLOW=$(grep -c "took.*ms (threshold: 50ms)" "$LOG_FILE" 2>/dev/null || echo "0")
            echo "  Slow collectors (>50ms): $SLOW"
            
            # Show average aggregation time if available
            if [ "$TOTAL" -gt 0 ]; then
                AVG=$(grep "Metrics aggregation completed\|Total metrics aggregation took" "$LOG_FILE" | \
                      sed -n 's/.*completed in \([0-9]*\)ms/\1/p; s/.*took \([0-9]*\)ms (threshold/\1/p' | \
                      awk '{ sum += $1; n++ } END { if (n > 0) print sum / n; else print "0" }')
                if [ "$AVG" != "0" ]; then
                    echo "  Average aggregation time: ${AVG}ms"
                fi
            fi
            
            echo ""
        fi
    done
    
    # Receiver stats if available
    if [ -f "$RECEIVER_LOG" ]; then
        echo -e "${GREEN}Receiver:${NC}"
        
        NEW_SESSIONS=$(grep -c "New session detected" "$RECEIVER_LOG" 2>/dev/null || echo "0")
        echo "  New sessions detected: $NEW_SESSIONS"
        
        PACKET_LOSS=$(grep -c "Packet loss detected" "$RECEIVER_LOG" 2>/dev/null || echo "0")
        echo "  Packet loss events: $PACKET_LOSS"
        
        REPLAYS=$(grep -c "Replay detected" "$RECEIVER_LOG" 2>/dev/null || echo "0")
        echo "  Replay attempts: $REPLAYS"
        
        echo ""
    fi
}

# Main command router
case "${1:-setup}" in
    setup)
        setup
        start_senders
        show_receiver_instructions
        echo ""
        echo -e "${YELLOW}To monitor logs: ./test-5-senders.sh monitor${NC}"
        echo -e "${YELLOW}To see summary:  ./test-5-senders.sh summary${NC}"
        echo -e "${YELLOW}To cleanup:      ./test-5-senders.sh cleanup${NC}"
        echo ""
        ;;
    
    receiver)
        start_receiver
        echo ""
        echo -e "${YELLOW}To monitor logs: ./test-5-senders.sh monitor${NC}"
        echo ""
        ;;
    
    monitor)
        monitor_logs
        ;;
    
    summary)
        show_summary
        ;;
    
    cleanup)
        cleanup
        ;;
    
    *)
        echo -e "${RED}Usage: $0 {setup|receiver|monitor|summary|cleanup}${NC}"
        echo ""
        echo "Commands:"
        echo "  setup    - Create configs and start 5 senders"
        echo "  receiver - Start the receiver process"
        echo "  monitor  - Tail logs in real-time"
        echo "  summary  - Show performance statistics"
        echo "  cleanup  - Kill all test processes"
        echo ""
        exit 1
        ;;
esac
