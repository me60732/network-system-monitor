#!/bin/bash

# Build script to verify metrics collection fixes
set -e

echo "=== Building network-system-monitor with metrics fixes ==="

cd /home/mark/Documents/in_cloud/Development/network-system-monitor

echo "Step 1: Build cosmic-applet..."
cargo build --package cosmic-applet 2>&1 | tee /tmp/cosmic-applet-build.log || {
    echo "ERROR: cosmic-applet build failed"
    exit 1
}

echo "Step 2: Build nmd-service..."
cargo build --package nmd-service 2>&1 | tee /tmp/nmd-service-build.log || {
    echo "ERROR: nmd-service build failed"
    exit 1
}

echo "Step 3: Build metrics-core..."
cargo build --package metrics-core 2>&1 | tee /tmp/metrics-core-build.log || {
    echo "ERROR: metrics-core build failed"
    exit 1
}

echo "=== All builds successful ==="
