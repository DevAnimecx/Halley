# ADR-005: Jerry Isolation

- **Status:** Accepted (isolation principle); exact mechanism
  (thread vs. process) is UNDECIDED pending engine decision
- **Date:** 2026-09-23

## Context

Jerry will eventually drive the browser autonomously using an external
LLM. Two failure modes must be prevented from day one:

1. **Availability:** a Jerry crash/panic/hang must not stop the user from
   browsing; a bad page must not corrupt Jerry.
2. **Authority:** Jerry (and the model behind it) must not gain ambient
   power over the user's machine or secrets, and must not be steered by
   hostile page content.

Both are much cheaper to guarantee structurally now than to retrofit.

## Decision

Jerry is a **separate trust and failure domain** connected to the browser
only through `halley-jerry-mcp`:

- **Communication:** exclusively MCP tool calls/responses — an explicit
  allowlist. Jerry has no path into `halley-core` internals
  (ADR-002 forbids the dependency).
- **No ambient authority:** no filesystem access, no raw sockets; LLM
  egress is limited to the user-configured BYOK provider; browser egress
  only via tools that reuse `halley-network`.
- **Server-side safety gate:** every tool call is classified READ /
  INTERACTION / SENSITIVE / DESTRUCTIVE *in code on the browser side*;
  DESTRUCTIVE requires explicit user confirmation regardless of what the
  model says.
- **Untrusted inputs:** page content and LLM output enter Jerry as framed
  data, never as executable instructions.
- **Crash containment:** Jerry runs behind a boundary that isolates panics
  from the browsing session. Initial implementation may be a supervised
  thread; a full separate process is the target if/when the threat model
  demands it. The thread-vs-process choice is **UNDECIDED** and will be
  revisited with the engine/process-model ADR (architecture §12).

## Reason

- MCP as the sole interface gives one auditable choke point for everything
  Jerry can do, with tiers enforceable server-side.
- Separating failure domains matches the PRD's isolation requirements
  without over-engineering IPC before there is a Jerry to isolate.
- Keeping keys out of Jerry's context means even a compromised model
  session cannot exfiltrate secrets it never sees.

## Consequences

- Jerry features must be expressed as MCP tools; anything that "needs"
  engine internals is a design bug.
- The safety gate and confirmation UX are blocking prerequisites for any
  autonomous-action milestone — they cannot be deferred behind the demo.
- A process-model ADR (IPC format, supervision, resource limits) is owed
  before Jerry ships; until then, thread-level containment plus the MCP
  boundary is the interim architecture.
- Jerry memory, when built, stores separately from the browser profile so
  it can be isolated and deleted independently (architecture §9).

## Alternatives considered

- **Jerry inside core as ordinary library calls:** rejected — collapses
  the authority and failure boundary; crash and injection blast radius
  become unbounded.
- **Enforcing safety only in the system prompt:** rejected — prompt
  injection makes model-side promises unenforceable.
- **Choosing full out-of-process IPC now:** rejected as premature —
  concrete IPC design depends on the undecided engine and process model;
  the MCP contract already defines the logical boundary.
- **Giving Jerry direct filesystem/network for "power user" tasks:**
  rejected — violates the PRD's isolation requirements; such tasks must be
  future explicit-confirmation tools, not ambient access.
