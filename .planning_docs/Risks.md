---
tags: [pipeline, risks, network-system-monitor]
created: 2025-07-13
---

# Risks: Network System Monitor

## Risk Register

| Risk | Severity | Likelihood | Mitigation |
|---|---|---|---|
| cosmic-lib applet API is unstable or poorly documented | 🟡 Medium | 🟡 Possible | Spike in week 1 — review docs and existing Cosmic applets for patterns |
| systemd service reliability on older hardware (ASRock mini) | 🟢 Low | 🟢 Unlikely | Test early; ASRock mini should handle a lightweight daemon fine |
| Network latency between machines makes real-time feel sluggish | 🟡 Medium | 🟡 Possible | Start with HTTP polling at reasonable interval; optimize later if needed |
| minimon-applet config format incompatible with our approach | 🟢 Low | 🟢 Unlikely | Review source code early; can extend or fork if needed |
| Authentication between machines becomes a security concern | 🟢 Low | 🟢 Unlikely | Home network threat model is low; token auth sufficient for MVP |
| cosmic-lib / iced framework changes break applet compatibility | 🟡 Medium | 🟡 Possible | Pin dependency versions; test upgrade path early |
| PR to cosmic-utils/cosmic-project-collection rejected or requires major changes | 🟢 Low | 🟢 Unlikely | Follow existing PR format (HoverDock example); use official templates |

**Severity**: 🔴 High (kills project) · 🟡 Medium (major rework) · 🟢 Low (manageable)  
**Likelihood**: 🔴 Likely · 🟡 Possible · 🟢 Unlikely

---

## Show Stoppers
Risks that, if they materialise, mean the project should be cancelled or fundamentally reconsidered.

1. cosmic-lib applet API doesn't support expandable windows / panel widgets — no way to build the core UX
2. systemd service approach is too complex for the target hardware (ASRock mini can't handle it)

## Assumptions Being Made
List the things you're assuming are true. If any of these turn out to be false, revisit the plan.

- Assuming all machines run Linux and have procfs/sysinfo access
- Assuming cosmic-lib applet API supports panel widgets with expandable content
- Assuming minimon-applet's design patterns can be adapted to cosmic-lib
- Assuming home network is stable enough for reliable data transport between machines
- Assuming ASRock mini will be set up and running before MVP testing

## Early Validation
What is the cheapest/fastest way to test the riskiest assumption before building anything?

1. Review minimon-applet source code — confirm UI patterns are adaptable, check config format
2. Review cosmic-lib applet API docs + existing Cosmic applets for panel widget examples
3. Set up ASRock mini and verify it can run a lightweight systemd service collecting metrics