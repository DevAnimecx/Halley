# ADR-001: Cargo Virtual Workspace

- **Status:** Accepted
- **Date:** 2026-09-23

## Context

Halley is a multi-subsystem product (browser core, engine, network,
privacy, AI agent, MCP protocol, extensions). The foundation milestone
needs a repository layout that compiles from day one, supports independent
testing of subsystems, and prevents the "one giant crate" failure mode
before any real feature work begins.

## Decision

Use a **virtual Cargo workspace** at the repository root (no root package)
whose members are the ten crates under `crates/`:

`halley-core`, `halley-engine`, `halley-network`, `halley-adblock`,
`halley-privacy`, `halley-jerry`, `halley-jerry-mcp`, `halley-jerry-opt`,
`halley-extensions`, `halley-common`.

Shared package metadata (`version`, `edition`, `license = "Apache-2.0"`,
`rust-version`) and shared lints (`unsafe_code = "deny"`, clippy `all`)
live in `[workspace.package]` / `[workspace.lints]`.

## Reason

- Virtual root keeps the dependency graph explicit: crates reference each
  other by path, and Cargo rejects cycles.
- One `cargo test --workspace` / `cargo clippy --workspace` gate covers
  everything — CI simplicity.
- Workspace-level `unsafe_code = "deny"` makes unsafe a repo-wide policy
  change rather than a per-crate oversight.
- Matches the PRD's modular mandate without inventing tooling.

## Consequences

- No single root binary target yet; an application crate (`halley-app` or
  similar) will be added when there is something to run. Root-level
  `tests/` and `benches/` directories are ignored by Cargo, so those live
  inside crates (root directories hold docs/fixtures only).
- Every new crate must opt into `[lints] workspace = true`.
- Version bumps apply workspace-wide.

## Alternatives considered

- **Monolithic single crate with modules:** rejected — violates the
  modularity requirement and allows forbidden dependencies to hide.
- **Root package + workspace:** rejected for now — there is no runnable
  product yet; an empty root package would be speculative.
- **Multiple independent repos:** rejected — cross-cutting refactors and
  atomic CI would suffer.
