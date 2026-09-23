# ADR-006: Browser Engine Selection

- **Status:** Accepted
- **Date:** 2026-09-23
- **Supersedes:** the “UNDECIDED” engine-selection part of
  [ADR-004](ADR-004-browser-engine-abstraction.md) (the abstraction
  boundary in ADR-004 remains Accepted and is unchanged).

## Context

Halley must render real web content behind the `halley-engine` boundary
(ADR-004). The PRD names **Obscura v0.2.1** as the intended engine, but
the first launchable-browser milestone requires a windowed desktop
webview that can be embedded today:

- Obscura is not published on crates.io, is not tagged/verified at
  v0.2.1 in public sources at the time of writing, and builds its own
  engine stack from source — unsuitable as an immediate build
  dependency for a compiling workspace milestone.
- The milestone needs: native window, address bar driving real
  navigation, back/forward/reload, page title, layout on resize — with
  honest tests and no fake rendering.

## Decision

1. **Selected:** embed the platform webview via **[wry](https://crates.io/crates/wry)**
   (WebView2 on Windows, WebKitGTK on Linux) with **[tao](https://crates.io/crates/tao)**
   as the native event loop/window.
2. The concrete types stay **inside `halley-engine` only**. Core, tests,
   and all other crates continue to depend solely on the
   `BrowserEngine` trait and Halley-owned types (`ChromeState`,
   `EngineEvent`, `EngineMessage`, `EngineError`).
3. Feature flags:
   - `native` (default) — wry + raw-window-handle backend.
   - `test-engine` — in-process `MockEngine` for integration tests with
     no window and no network.
4. **Obscura remains a candidate** for a later migration if/when it is a
   real, auditable, windowed dependency. This ADR does not reject
   Obscura on merit; it records that it cannot satisfy this milestone’s
   build-and-run criteria today.

## Reason

- Platform webviews are the only realistic way to ship a launchable
  browser without vendoring a full engine build in this milestone.
- wry/tao are the de-facto Rust-native pair (used by Tauri), mature,
  dual-licensed (Apache-2.0 compatible), and expose navigation,
  IPC, and layout hooks sufficient for the minimal chrome.
- Isolating wry behind `BrowserEngine` keeps the ADR-004 swap story
  intact: replacing wry later should touch `halley-engine` (and this
  ADR), not the rest of the workspace.
- Choosing honestly over the PRD name beats faking Obscura-shaped APIs.

## Consequences

- Rendering fidelity, process sandboxing, and fingerprint surfaces are
  those of **WebView2 / WebKitGTK**, not a Halley-controlled engine —
  privacy-model §6/§7 work must target the platform webview’s knobs.
- `unsafe` is still denied in Halley crates; wry/tao may contain `unsafe`
  internally as third-party code (out of workspace lint scope).
- Default tests (`test-engine` / unit) need no display; real-window e2e
  is `#[ignore]`d and runs only with an explicit flag.
- Linux CI must install WebKitGTK/GTK development packages to build the
  `native` feature; Windows CI uses the preinstalled WebView2 runtime.
- A future engine swap ADR must re-evaluate Obscura, Servo, or a
  custom stack against this one’s lessons (windowed embedding, IPC
  chrome, native history).
- Stop/abort of page loads is **NOT IMPLEMENTED** (wry 0.57 exposes no
  `stop()`); the chrome omits a Stop control rather than faking one.

## Alternatives considered

- **Obscura (PRD name) now:** rejected for this milestone — not a
  verifiable crates.io dependency; building V8/engine from source does
  not meet “compiling, runnable workspace” criteria. Revisit when it
  does.
- **CEF (Chromium Embedded Framework):** rejected — heavy binary
  footprint, awkward licensing/packaging, complex upgrade story for a
  privacy-first MVP.
- **Servo:** rejected for now — embedding/windowed story still not
  stable enough for a launchable single-tab MVP; revisit as engine
  landscape moves.
- **Implementing a renderer in Halley:** rejected — years of work;
  violates “no premature complexity” for this milestone.
- **No abstraction, call wry from core:** rejected — would violate
  ADR-004 and make any future engine swap a repo-wide rewrite.
