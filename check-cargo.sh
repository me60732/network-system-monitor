#!/bin/bash
cd /home/mark/Documents/in_cloud/Development/network-system-monitor
cargo check --package cosmic-applet 2>&1 | head -50
