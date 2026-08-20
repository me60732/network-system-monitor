---
tags: [pipeline, brief, network-system-monitor]
status: 🔍 Exploring
tagline: "A Cosmic desktop applet that monitors all machines on your network from one panel."
created: 2025-07-13
---

# Brief: Network System Monitor

## The Spark
You have multiple Linux machines on the network — desktop, old laptop, ASRock mini (not running yet but wanted to find a use for it), and the Spark arriving soon. Right now you have to SSH into each machine to look up CPU/memory usage etc. You want a GUI system monitor that can monitor any machine you install it on.

## What Is It
A Cosmic desktop applet that shows your desktop's system usage in the panel (like minimon-applet does now), and when clicked opens a window displaying usage for all other machines on the network. Each machine runs a systemd service sending data to the desktop. Eventually expands to individual system monitors per machine, not just the desktop.

## The Problem It Solves
Eliminating the need to SSH into each machine just to check CPU/memory/disk/network stats. One glance at the panel gives you visibility across your entire fleet of machines. The ASRock mini finally gets a purpose as part of this monitoring network.

## Who Is It For
You — the primary user who manages multiple Linux machines and wants quick visibility without SSHing into each one.

## Why Now
The Spark is arriving soon, adding another machine to monitor. The ASRock mini has been sitting idle wanting a use. minimon-applet already works well for single-machine monitoring but doesn't do network-wide — this fills that gap.

## First Instinct on Tech
Build it in Rust using the same GUI framework as Cosmic (cosmic-lib / iced). Keep it consistent with the Cosmic desktop itself. Use minimon-applet's design and configuration as a reference to steal much of the UI/app design from. Each machine runs a systemd service sending data to the desktop.

## Resolved Decisions
- **Protocol**: UDP push with rkyv serialization, single-direction no retries ✅ (confirmed by Captain in lavish v2 feedback)
- **Authentication**: mTLS/certificate auth ✅ (self-signed certs, install scripts handle distribution)
- **Metrics**: CPU/memory/disk/network/uptime/GPU/VRAM/temp all in MVP ✅
- **UI Layout**: Grid layout confirmed (columns per metric, rows per machine) ✅
- **Panel Widget**: Single-line format sufficient ✅
- **Expand Behavior**: Click-to-expand confirmed ✅
- **Config Menu**: Checkbox selection confirmed ✅
- **Status Indicators**: ●/○ symbols for Online/Offline/Pending confirmed ✅
- **Color Thresholds**: 60%/80% thresholds confirmed ✅
- **Tech Stack**: Rust + cosmic-lib/iced (v1.3-v1.4 well-documented) ✅
- **Phased Approach**: Approved ✅

## Resolved Decisions (v2 Feedback)
- **Protocol**: UDP push with rkyv serialization ✅ — single-direction, no retries. Each remote machine pre-configured with desktop host IP. Machine ID included in packet for auto-registration. (Confirmed by Captain)
- **Authentication**: mTLS/certificate auth ⚠️ re-evaluating — unusual over UDP; may pivot to pre-shared token/HMAC for MVP. Certificates deferred.
- **Auto-discovery**: Not needed ✅ — each remote machine is pre-configured with host IP. New machines auto-register when first packet arrives. (Confirmed by Captain)
- **minimon-applet config**: Extend TOML format ✅ — adapt existing schema rather than creating new one. (Confirmed by Captain)
- **ASRock Mini**: Not blocking ✅ — using Pluto + Spark for testing instead.
- **Firewall scripts**: Deferred ⏭️ — keep firewall configuration separate as security precaution.

---

## Cosmic Utils Submission Criteria
- Open PR against `cosmic-utils/cosmic-project-collection` — site's project list auto-generated from that repo's data files
- Add entry to appropriate RON file (`applets.ron`, etc.) with fields: name, description, repo URL, image (can be None or raw.githubusercontent screenshot URL)
- Use official templates: `pop-os/cosmic-applet-template` for applets
- justfile recipes required: build-release, install, vendor, check (clippy), check-json
- Flatpak support via cosmic.flatpakrepo source for app store visibility
- Installation paths follow standard Linux conventions (/usr/bin, /usr/share/appdata/, etc.)
- Optional: Include full source code in PR

---
*Next step: flesh out [[Scope.md]] and [[Architecture.md]] to define MVP boundaries and tech decisions.*