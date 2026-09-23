# ADR-007: Tab lifecycle, popup policy, and session model

Status: **IMPLEMENTED** (in-memory multi-tab session; no disk persistence).

## Context

The engine spike (ADR-006) shipped a single content web view behind
chrome. Multi-tab browsing (PRD FR-1.2) needs:

- Stable tab identity independent of strip position.
- Explicit session lifecycle so future restore/agent attach points are
  gated without ad-hoc flags.
- A policy for `window.open` / `target=_blank` without creating native
  OS windows (which would bypass Halley chrome).
- Honest handling of operations the platform engine does not support
  (`stop()`).

## Decision

1. **Tab identity:** `TabId(u64)` assigned monotonically per
   `TabManager`; never reused within a session. Chrome IPC carries the
   decimal string form; malformed ids are ignored (not guessed).
2. **Page binding:** each open tab owns exactly one engine `PageId`.
   Closing a tab destroys its page. Inactive pages stay alive and are
   hidden (`set_visible(false)`) so form/JS state survives tab switches.
3. **Strip invariant:** the tab list is never empty after construction.
   Closing the last tab opens a fresh tab at `config.browser.new_tab_url`
   (default `about:blank`).
4. **Session lifecycle:** `Created → Initializing → Ready ⇄ Running →
   Closing → Closed`. Commands are accepted only in `Ready`/`Running`.
5. **Session snapshots:** `SessionSnapshot` with `schema_version: 1`
   serializes open-tab URLs/titles/active index via serde JSON. Restore
   rebuilds a session *shell*; the caller recreates engine pages. Wrong
   schema versions are **rejected**, not partially applied. No disk
   auto-save (later milestone).
6. **Popup / new-window policy:** the engine **denies** native new
   windows (`NewWindowResponse::Deny`) and reports
   `EngineEvent::NewWindowRequested { page, url }`. Core opens a Halley
   tab with that URL (scheme-filtered by the engine to http/https/about/
   file). See also ADR-006 containment rules.
7. **Stop:** `BrowserCommand::Stop` / wry `stop()` returns typed
   `CommandOutcome::NotSupported` / `EngineError::Unsupported`. The
   chrome UI does **not** render a Stop control (no fake affordance).
8. **Keyboard shortcuts:** implemented in `assets/chrome.html`
   (Ctrl+T/W/Tab/Shift+T, Ctrl+L/F6, Alt+←/→, F5/Ctrl+R, Ctrl+1..9).
   Limitation **documented**: when the content page holds focus,
   WebView2 may consume accelerator keys before chrome sees them. We do
   not fake a native accelerator layer.
9. **Closed-tab stack:** LIFO, bounded by `config.browser.max_closed_tabs`
   (default 10). Reopen allocates a *new* `TabId` with the saved URL.

## Reason

- Typed ids and 1:1 tab↔page mapping keep event routing unambiguous
  (title/load events carry `PageId`).
- Schema-versioned snapshots make future session restore fail closed
  on incompatible data.
- Denying native popups keeps every surface inside Halley chrome
  (security + multi-tab UX).
- Honest `NotSupported` for Stop matches AGENTS.md §2.1 (never fake).

## Consequences

- Every tab is a real platform child web view → memory grows with open
  tabs (PRD budget `<30 MB/tab` is **not measured** this milestone;
  see `docs/performance-baseline.md`).
- Keyboard shortcuts are chrome-scoped until a native accelerator
  layer exists (future ADR if we add one).
- Session JSON is foundation only: no crash recovery yet.
- Closing other tabs / to-the-right reorders operations must destroy
  engine pages *before* dropping tab records (implementation detail in
  `Browser::close_other_tabs` / `close_tabs_to_right`).

## Alternatives considered

- **Index-based tabs:** rejected — reorder/close makes indices lie.
- **Destroy inactive pages and reload on focus:** rejected for this
  milestone — loses form state and contradicts FR-6.6 lazy-background
  intent (we keep pages alive hidden instead).
- **Native popup windows:** rejected — escapes chrome, breaks session
  model, complicates security review.
- **proptest for invariants:** rejected for now — no new dep; a
  deterministic LCG suite in `tests/property_tabs.rs` covers the same
  invariants (AGENTS.md §8).

## Related

- ADR-004 (engine abstraction), ADR-006 (wry/tao selection).
- PRD FR-1.2 (multi-tab), FR-6.6 (background tabs), FR-6.2 (memory).
