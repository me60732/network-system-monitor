---
tags: [pipeline, roadmap, network-system-monitor]
created: 2025-07-13
updated: 2026-08-27
---

# Roadmap: Network System Monitor

## Current Status: MVP Complete 🚀

**As of 2026-08-27:**
- ✅ Phase 1 MVP is **working end-to-end**
- ✅ metrics-core, nmd-service, cosmic-applet all functional
- ✅ HMAC-SHA256 authentication working
- ✅ Multi-machine aggregation and display
- ✅ Installation scripts created
- 🚧 Production hardening in progress (see PRODUCTION-READINESS.md)

**Ready for:** Home network deployment and testing

---

## MVP — The Smallest Useful Thing
What is the absolute minimum that makes this worth having?

**MVP includes:**
- ✅ Cosmic applet showing desktop CPU/memory/disk/network/uptime/GPU/VRAM/temp stats in panel
- ✅ Click-to-expand window displaying all configured remote machines' stats (grid layout confirmed)
- ✅ systemd service on each remote machine collecting and sending metrics to desktop
- ✅ Config menu for adding/removing machines and choosing which metrics per machine
- ⚠️ Ability to open local system monitor from the applet (button exists, wiring TODO)

**MVP achieved!** Now focusing on production hardening.

---

## Phase 1 — MVP: Applet + Network Visibility ✅ COMPLETE
**Goal**: Get it working for the primary use case  
**Status**: ✅ Done — All core functionality working

| Task | Status | Notes |
|---|---|---|
| Review minimon-applet source code | ✅ Complete | Patterns adapted for cosmic-lib |
| Create `metrics-core` crate | ✅ Complete | CPU, memory, disk, network, GPU, temp all working |
| Build systemd service binary | ✅ Complete | UDP push with rkyv + HMAC-SHA256 |
| Build Cosmic applet panel widget | ✅ Complete | Shows desktop + aggregated remote stats |
| Build click-to-expand window | ✅ Complete | Grid layout with all machines |
| Config menu | ✅ Complete | Per-sensor toggles, content ordering, percentage modes |
| Test with Pluto + Spark | ⚠️ TODO | Have install scripts, need live deployment test |
| Provide install scripts | ✅ Complete | Auto-install with HMAC setup |

**Achievements:**
- Graduated ring chart colors (green → orange → red by threshold)
- VRAM display with GB/percentage toggle
- Combined CPU + temperature display
- Zero compiler warnings
- Security hardening (systemd restrictions, HMAC auth)

---

## Phase 1.5 — Production Hardening 🚧 IN PROGRESS
**Goal**: Make it reliable and deployable for end users  
**Status**: Installation scripts done, working through remaining TODOs

| Task | Status | Priority | Notes |
|---|---|---|---|
| Installation scripts | ✅ Complete | HIGH | install.sh + uninstall.sh working |
| HMAC key generation | ✅ Complete | HIGH | Automated in install script |
| Deployment documentation | ✅ Complete | HIGH | DEPLOYMENT.md created |
| Offline machine detection | ⏳ TODO | **CRITICAL** | Machines never show as offline (#2, #5) |
| Config persistence testing | ⏳ TODO | **CRITICAL** | Ensure saves work (#1) |
| Update test code | ⏳ TODO | HIGH | Tests disabled after packet refactor (#3) |
| Config validation | ⏳ TODO | HIGH | Helpful errors for bad config (#8) |
| Desktop install method | ⏳ TODO | HIGH | Currently requires --test-mode or cargo run |
| Launch system monitor | ⏳ TODO | MEDIUM | Wire up button (#6) |
| Per-core CPU display | ⏳ TODO | LOW | Nice-to-have (#4) |
| Error handling polish | ⏳ TODO | MEDIUM | Better error messages (#7) |

**Target:** Complete by Week 2 (2 weeks from now)

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