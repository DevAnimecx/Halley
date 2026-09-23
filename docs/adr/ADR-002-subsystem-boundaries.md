# ADR-002: Subsystem Boundaries

- **Status:** Accepted
- **Date:** 2026-09-23

## Context

The PRD requires a modular architecture with separate subsystems for
orchestration, engine, network, adblock, privacy, the Jerry agent, its MCP
protocol, optimization, and extensions. Without an explicit dependency
policy, future features (especially Jerry and privacy enforcement) will
naturally reach across concerns and create cycles.

## Decision

Define the crates and a **strict, one-directional dependency graph**:

```
halley-core
  ├── halley-engine
  ├── halley-network ──► halley-adblock
  ├── halley-privacy
  ├── halley-jerry ──► halley-jerry-mcp
  │                └──► halley-jerry-opt
  └── halley-extensions
              │
              ▼
        halley-common        (bottom: depends on no Halley crate)
```

Rules:

1. Dependencies point downward only; no cycles (enforced by Cargo).
2. `halley-common` holds only shared primitives (error strategy, config
   types, logging) and business logic is forbidden there.
3. `halley-adblock` is reachable only from `halley-network` — filtering is
   a network-pipeline concern, not a rendering concern.
4. Jerry (`halley-jerry*`) never depends on `halley-core` or
   `halley-engine`; it reaches the browser exclusively through
   `halley-jerry-mcp` types.
5. Changing the graph requires a new ADR.

## Reason

- Makes privacy auditable: all egress must sit in `halley-network`,
  reachable only downward from core.
- Keeps Jerry's authority narrow and testable in isolation.
- Cargo enforces the policy mechanically — review is a backup, not the
  primary control.
- Each crate's README/doc header states its responsibility, giving future
  agents a clear placement rule for new code.

## Consequences

- Some duplication may appear before code is promoted into
  `halley-common`; prefer a little duplication over a wrong abstraction.
- Cross-cutting features (e.g. privacy checks inside the engine) must be
  passed down as values/traits from core rather than reaching sideways.
- Jerry features that appear to need engine internals must be redesigned
  as MCP tools.

## Alternatives considered

- **Plugin/hook architecture between all crates:** rejected — premature
  complexity for a genesis codebase.
- **Jerry depending on halley-core directly:** rejected — destroys the
  isolation boundary the security model depends on.
- **Adblock as a rendering-stage filter:** rejected — network-level
  blocking is the PRD's stated approach.
