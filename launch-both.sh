#!/bin/bash
# Launch both sender and receiver for testing

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Killing any existing processes..."
pkill -f nmd-service
pkill -f cosmic-applet
sleep 1

echo "Starting sender in background..."
cd "$PROJECT_ROOT"
./run-sender.sh > /tmp/sender.log 2>&1 &
SENDER_PID=$!
echo "Sender PID: $SENDER_PID"

sleep 2

echo "Starting receiver in background..."
./run-receiver.sh > /tmp/receiver.log 2>&1 &
RECEIVER_PID=$!
echo "Receiver PID: $RECEIVER_PID"

sleep 3

echo ""
echo "Services launched. Monitor with:"
echo "  tail -f /tmp/sender.log"
echo "  tail -f /tmp/receiver.log"
echo ""
echo "To stop:"
echo "  kill $SENDER_PID $RECEIVER_PID"
