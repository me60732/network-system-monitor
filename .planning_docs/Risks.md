---
tags: [pipeline, risks, network-system-monitor]
created: 2025-07-13
---

# Risks: Network System Monitor

## Risk Register

| Risk | Severity | Likelihood | Mitigation |
|---|---|---|---|
| cosmic-lib applet API is unstable or poorly documented | 🟡 Medium | 🟡 Possible | Spike in week 1 — review docs and existing Cosmic applets for patterns |
| UDP packet loss / network unreliability between machines | 🟡 Medium | 🟡 Possible | Push model tolerates this — missed packets just resend next cycle. No retries needed.
| Network latency between machines makes real-time feel sluggish | 🟡 Medium | 🟡 Possible | Start with HTTP polling at reasonable interval; optimize later if needed |
| minimon-applet config format incompatible with our approach | 🟢 Low | 🟢 Unlikely | Review source code early; can extend or fork if needed |
| UDP auth strategy (mTLS unusual over UDP) | 🟡 Medium | 🟢 Unlikely | May pivot to pre-shared token/HMAC if mTLS proves unsuitable. Certificates deferred.
| cosmic-lib / iced framework changes break applet compatibility | 🟡 Medium | 🟡 Possible | Pin dependency versions; test upgrade path early |
| PR to cosmic-utils/cosmic-project-collection rejected or requires major changes | 🟢 Low | 🟢 Unlikely | Follow existing PR format (HoverDock example); use official templates |

**Severity**: 🔴 High (kills project) · 🟡 Medium (major rework) · 🟢 Low (manageable)  
**Likelihood**: 🔴 Likely · 🟡 Possible · 🟢 Unlikely

---

## Show Stoppers
Risks that, if they materialise, mean the project should be cancelled or fundamentally reconsidered.

1. cosmic-lib applet API doesn't support expandable windows / panel widgets — no way to build the core UX
2. rkyv serialization proves incompatible across Rust versions on different machines — would need fallback protocol (Cap'n Proto/flatbuffers)

## Assumptions Being Made
List the things you're assuming are true. If any of these turn out to be false, revisit the plan.

- Assuming all machines run Linux and have procfs/sysinfo access
- Assuming cosmic-lib applet API supports panel widgets with expandable content
- Assuming minimon-applet's design patterns can be adapted to cosmic-lib
- Assuming home network is stable enough for reliable data transport between machines
- Assuming UDP push model is sufficient (no retries needed — missed packets just resend next cycle)

## Early Validation
What is the cheapest/fastest way to test the riskiest assumption before building anything?

1. Review minimon-applet source code — confirm UI patterns are adaptable, check config format
2. Review cosmic-lib applet API docs + existing Cosmic applets for panel widget examples (refs: libcosmic GitHub, libcosmic docs, cosmic-applet-template)
3. Prototype rkyv serialization roundtrip with minimal metrics data to validate cross-machine compatibility