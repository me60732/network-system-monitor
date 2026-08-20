---
tags: [pipeline, goals, network-system-monitor]
created: 2025-07-13
---

# Goals: Network System Monitor

## Primary Goal
One sentence. What is the single most important outcome?

Eliminate SSHing into each machine to check stats — one glance at the Cosmic panel gives visibility across my entire fleet of Linux machines.

## Success Metrics
How do you measure success? Be specific — vague goals are unverifiable.

| Metric | Target | How Measured |
|---|---|---|
| Panel applet shows desktop stats instantly | < 1s load time | Manual test |
| Click-to-expand window shows all remote machines | All configured machines visible within 2s | Manual test |
| Remote systemd service runs reliably | No crashes over 7 days of continuous operation | Observation |
| Config menu works for adding/removing machines | Add/remove machine in < 30 seconds | Manual test |

## Secondary Goals
What else would make this worthwhile, beyond the primary goal?

1. Pluto (file server) + Spark both get integrated into the monitoring network, giving existing idle hardware new purpose
2. UI consistent with Cosmic desktop — feels native, not bolted-on
3. Config format compatible with or extendable from minimon-applet's config
4. Eventually expand to individual per-machine system monitors (direction, not MVP)
5. Future version mockup approved — process-level management evolution (Phase 5+)
6. Submit to cosmic-utils.org for community visibility and app store distribution (Phase 4)

## Non-Goals
What outcomes are explicitly NOT what this project is trying to achieve?
(Prevents scope creep when someone says "while we're at it, why not also...")

- Not trying to build a commercial monitoring product
- Not trying to support Windows or macOS remote machines
- Not trying to provide alerting/notification (visibility only)
- Not trying to replace existing system monitors on each machine

## Timeline Expectations
Rough expectations — not a commitment, just a sanity check.

| Phase | Expected Effort |
|---|---|
| MVP (applet + 2 remote machines) | 3-4 weeks |
| v1 (harden config, more metrics) | 2 weeks |
| Per-machine monitors (direction) | TBD — separate project when MVP proves useful