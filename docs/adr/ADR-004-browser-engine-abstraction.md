# ADR-004: Browser Engine Abstraction

- **Status:** Accepted (boundary). Engine **selection** is now decided in
  [ADR-006](ADR-006-browser-engine-selection.md) (wry + tao); that part of
  this ADR’s “UNDECIDED” note is superseded. The crate boundary itself is
  unchanged and still Accepted.
- **Date:** 2026-09-23

## Context

Halley must render real web content. The PRD does not select a rendering
engine, and engine choice drives process model, fingerprint surface,
footprint, licensing, and how much of the stack Halley must build itself.
Choosing an engine now — before any rendering milestone — would be
speculative.

## Decision

1. All engine interaction is isolated behind the `halley-engine` crate.
   No other crate may name a concrete engine type.
2. The concrete engine and embedding technique are **UNDECIDED** and will
   be decided in a dedicated ADR when the rendering milestone begins.
3. Until that ADR, `halley-engine` contains only its subsystem identity
   and documented boundary — no speculative traits or adapter skeletons.

## Reason

- The workspace compiles today without pre-committing to an engine the
  PRD never chose (accuracy over speed).
- The crate boundary is the decision that matters now: it forces the
  engine-adapter seam to exist when the engine arrives, instead of engine
  types leaking into core/network/jerry code.
- Avoiding speculative traits honors the "no premature complexity"
  principle — an invented trait would likely be wrong before real render
  requirements exist.

## Consequences

- Rendering, navigation, and tab UI are deferred by definition.
- The future engine ADR must evaluate at minimum: embedding an existing
  engine vs. building on a Rust-native engine, license compatibility with
  Apache-2.0 distribution, sandbox/process story, maintenance burden, and
  fingerprint-resistance headroom (privacy-model §7 depends on this).
- Engine failures must eventually be caught at this seam so page crashes
  don't unwind the process (security-model §2.1).

## Alternatives considered

- **Pre-select an engine now (e.g. Servo, WebView2, CEF):** rejected —
  PRD leaves it open; premature commitment with major licensing and
  process-model implications.
- **Define the full `Engine` trait now:** rejected — any trait designed
  without a real renderer is guesswork; the trait belongs to the engine
  ADR.
- **Scattering engine calls through core:** rejected — would make the
  eventual engine swap a repo-wide rewrite.
