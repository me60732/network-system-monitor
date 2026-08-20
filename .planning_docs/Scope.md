---
tags: [pipeline, scope, network-system-monitor]
created: 2025-07-13
---

# Scope: Network System Monitor

## One-Line Definition
Complete this sentence: *"This project succeeds when the Cosmic panel applet shows my desktop's system stats at a glance and opens a window displaying all other machines on the network — no SSH required."*

## In Scope (MVP)
What is definitely included in the first working version?

- Cosmic applet showing desktop machine usage in the panel (like minimon-applet does now)
- Click-to-expand panel/window showing all other machines' system stats
- Systemd service on each remote machine that collects and sends metrics to the desktop
- Configuration menu to choose what metrics to monitor per machine
- Ability to open the local system monitor from the applet
- minimon-applet's UI/design as reference — steal much of it for consistency
- cosmic-app-template layout patterns as design reference

## Explicitly Out of Scope
This section is as important as what's IN. List things that seem related but are NOT being built.

- Individual per-machine system monitors (deferred to Phase 3+)
- Alerting/notification system (deferred — MVP is visibility, not action)
- Alerting/notification system (deferred — MVP is visibility, not action)
- Mobile app or remote access from outside the network
- Multi-user / shared monitoring across a team

## Why These Boundaries
MVP focuses on the applet and network-wide visibility with full metrics coverage (CPU, memory, disk, network, uptime, temperature, GPU, VRAM). Individual per-machine monitors are the eventual direction but add complexity that blocks the core value proposition. Alerting is a separate problem from visibility. Mobile access is out of scope for a home-network tool.

## Definition of Done
How do you know MVP is complete? What does a working version look like in concrete terms?

- Applet installed on desktop, shows CPU/memory/disk/network/uptime/GPU/VRAM/temp stats in panel
- Clicking applet opens window showing all configured remote machines' stats
- Each remote machine has systemd service running and sending data to desktop
- Config menu lets you add/remove machines and choose which metrics per machine (checkbox selection confirmed)
- Status indicators: ●/○ symbols for Online/Offline/Pending confirmed
- Color thresholds at 60%/80% for metric bars confirmed
- Local system monitor can be opened from the applet

## Dependencies
What must be true before this project can start?

- [ ] ASRock mini setup and running (needed as first remote test target)
- [ ] Spark arriving and network-configured (second remote target)
- [ ] Cosmic desktop environment available on desktop machine
- [ ] minimon-applet source code reviewed for UI reference patterns

## Submission Requirements (Phase 4+)
- Open PR against `cosmic-utils/cosmic-project-collection` with entry in `applets.ron`
- Add name, description, repo URL, screenshot image to RON file
- Use official templates: `pop-os/cosmic-applet-template`
- Ensure justfile recipes work: build-release, install, vendor, check (clippy), check-json
- Add flatpak to cosmic.flatpakrepo source for app store visibility