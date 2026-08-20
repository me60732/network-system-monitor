---
tags: [pipeline, roadmap, network-system-monitor]
created: 2025-07-13
---

# Roadmap: Network System Monitor

## MVP — The Smallest Useful Thing
What is the absolute minimum that makes this worth having?

**MVP includes:**
- Cosmic applet showing desktop CPU/memory/disk/network/uptime/GPU/VRAM/temp stats in panel
- Click-to-expand window displaying all configured remote machines' stats (grid layout confirmed)
- systemd service on each remote machine collecting and sending metrics to desktop
- Config menu for adding/removing machines and choosing which metrics per machine
- Ability to open local system monitor from the applet

**MVP excludes everything else.**

---

## Phase 1 — MVP: Applet + Network Visibility
**Goal**: Get it working for the primary use case  
**Done when**: Panel shows desktop stats, click opens window showing remote machines' stats, systemd services running on at least 2 remote targets (ASRock mini + one other)

| Task | Notes |
|---|---|
| Review minimon-applet source code for UI patterns and config format | Steal what works, adapt to Cosmic applet API |
| Create `metrics-core` crate — shared data collection between desktop and remote | CPU, memory, disk, network stats via sysinfo + procfs |
| Build systemd service binary — UDP push with rkyv encoding | Push model confirmed ✅. Machine ID included in packet for auto-registration. No retries — resend next cycle.
| Build Cosmic applet panel widget — shows desktop stats at a glance | Use minimon-applet design as reference; cosmic-lib API for panel widget |
| Build click-to-expand window — displays all remote machines' stats | Grid layout, one row per machine, same metrics as panel |
| Config menu — add/remove machines, choose metrics per machine | TOML-based config; UI to edit it |
| Test with Pluto (file server) + Spark | Both machines available. No dependency on ASRock Mini.
| Provide install scripts for remote machines | Auto-generate self-signed certs, install systemd unit + binary in one script, auto-generate config.toml |

---

## Phase 2 — Harden: More Metrics + Reliability
**Goal**: Make it reliable and usable by others  
**Done when**: All configured metrics work reliably, config is robust, handles network failures gracefully

| Task | Notes |
|---|---|
| Add more metrics (swap, uptime, load average, etc.) | Expand what sysinfo/procfs provides |
| Handle remote machine offline / network failure gracefully | Show "unavailable" status, retry logic |
| Improve config UX — validate TOML, show errors clearly | Prevent misconfiguration |
| Re-evaluate UDP auth strategy | mTLS over UDP unusual. May pivot to pre-shared token/HMAC for MVP. Certificates deferred.

---

## Phase 3 — Expand: Per-Machine System Monitors
**Goal**: Add the next most valuable capability  
**Done when**: Each remote machine has its own system monitor applet, not just desktop monitoring all machines

| Task | Notes |
|---|---|
| Adapt applet for deployment on each remote machine | Same codebase, different config mode (local-only vs network) |
| Cross-machine navigation — click a remote in the window to open its local monitor | Deep link between monitors |
| Unified config across all machines | One source of truth for what's monitored where |

---

## Phase 4 — Submit to Cosmic Utils
**Goal**: List in cosmic-utils.org for community visibility and app store distribution
**Done when**: PR merged against `cosmic-utils/cosmic-project-collection`, flatpak added to cosmic.flatpakrepo

| Task | Notes |
|---|---|
| Open PR against `cosmic-utils/cosmic-project-collection` with entry in `applets.ron` | Add name, description, repo URL, screenshot image |
| Add flatpak to cosmic.flatpakrepo source for app store visibility | Follow cosmic-applet-template's justfile install recipe |
| Ensure all justfile recipes work: build-release, install, vendor, check (clippy), check-json | Required by cosmic-utils standards |

---

## Explicitly Deferred
Things that will not be in any of the above phases. Park them here so they stop coming up.

- Alerting/notification system — separate project, different problem space
- Alerting/notification system — separate project, different problem space
- Mobile app or remote access from outside the network — out of scope for home-network tool
- Multi-user / shared monitoring across a team — not the primary use case