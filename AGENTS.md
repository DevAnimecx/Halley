# AGENTS.md — Engineering Constitution for Halley

**Read this file completely before making any change to this repository.**
If a proposed change conflicts with this document, stop and ask the human
maintainer instead of proceeding.

This is the permanent engineering constitution for all AI coding agents
working on Halley. It overrides convenience, speed, and looking complete.

---

## 1. What Halley is

Halley is a **privacy-first browser written in Rust** with a built-in
autonomous AI browser agent called **Jerry**.

- Halley: lightweight Rust browser — web rendering, multi-tab browsing,
  network-level ad/tracker blocking, fingerprint resistance, local encrypted
  storage, WASM extensions, reproducible cross-platform builds.
- Jerry: an autonomous agent that plans and executes multi-step browser
  tasks using a **bring-your-own-key (BYOK)** LLM provider, with local
  memory, model routing, token optimization, and safety gating — driven
  through an MCP-based tool protocol.

The product source of truth is the project PRD plus the documents in
`docs/`. When they disagree with this file on *process*, this file wins; when
they disagree on *product scope*, the PRD wins.

**Current status: multi-tab browser session + Jerry chat foundation.**
The repository compiles as a workspace, opens a native window with a tab
strip and address-bar chrome, renders each tab as its own platform web
view (wry; ADR-006), supports new/close/activate/reorder/reopen and typed
browser commands, hosts a Jerry chat panel (BYOK providers, local
credential/conversation files, streaming completions, read-only tool
contracts — Prompt #5 / ADR-010; **no** autonomous control or tool
executor), and has tests + CI. Most product features (network choke
point, privacy enforcement, history UI, Jerry planner/tools, adblock,
extensions) are still absent. Never present planned features as done.

## 2. Core principles (non-negotiable)

1. **Never fake functionality.** No fake engines, fake AI responses, fake
   MCP execution, hardcoded "results", or UI that pretends a backend
   exists. If something is not implemented, say so in code, docs, and UI.
2. **No premature complexity.** Add a dependency, abstraction, module, or
   trait only when the current milestone actually needs it. No speculative
   code. Prefer the simplest thing that honestly works.
3. **Modular architecture.** Each crate has one responsibility and a
   one-directional dependency graph (see §3). No circular dependencies.
4. **Security first.** Never trade security for development convenience
   (see §5).
5. **Privacy by architecture.** Unwanted network communication must be hard
   to add, not merely forbidden by prose (see §6).
6. **Verification over claims.** You are done only when commands pass, not
   when you believe the code is correct.

## 3. Architecture and dependency rules

Workspace layout (virtual workspace, root `Cargo.toml`):

```
crates/
  halley-common/      shared primitives: error strategy, config types, logging
  halley-core/        application orchestration and browser-level state (top)
  halley-engine/      browser engine abstraction
  halley-network/     network infrastructure (all outbound traffic funnels here)
  halley-adblock/     ad/tracker filtering (sits behind the network pipeline)
  halley-privacy/     privacy policies and enforcement points
  halley-jerry/       the AI agent (planner, execution, memory)
  halley-jerry-mcp/   MCP tool protocol between Jerry and the browser
  halley-jerry-opt/   token optimization and model routing
  halley-extensions/  WASM extension sandbox
ui/                   frontend code (none yet)
docs/                 architecture, security, privacy, ADRs
tests/                cross-crate test docs/fixtures (crate tests live in crates/)
benches/              benchmark docs (benches live in crates when they exist)
scripts/              local verification scripts (mirror CI)
assets/               static assets
.github/workflows/    CI
```

Allowed dependency direction (lower may not depend on higher):

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
        halley-common        (bottom: depends on nothing in this repo)
```

Rules:

- Every crate may depend on `halley-common`. `halley-common` depends on no
  other Halley crate.
- `halley-adblock` must be reachable from `halley-network` (filtering is a
  network-pipeline concern), not from rendering.
- Jerry (`halley-jerry*`) talks to the browser **only** through
  `halley-jerry-mcp` tool abstractions — never by reaching into
  `halley-core` internals.
- To change this graph, write an ADR first.
- Each crate exposes `pub const SUBSYSTEM: &'static str` for diagnostics.

## 4. Coding standards

- Idiomatic modern Rust, edition 2021, stable toolchain only
  (see `rust-toolchain.toml`).
- `cargo fmt` and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  must pass before you report completion.
- `unsafe` is **denied at the workspace level**
  (`[workspace.lints.rust] unsafe_code = "deny"`). If `unsafe` becomes truly
  unavoidable, stop, document the justification in an ADR, and get human
  approval before flipping the lint for that crate.
- Document all public items with `///` (or `//!` at module level). The docs
  must state what is *not* implemented when relevant.
- Errors: use `Result`-based handling. Subsystems define their own concrete
  error types; convert to `halley_common::Error` only at shared boundaries.
  Error messages must never contain secrets.
- Small modules, explicit interfaces, meaningful names. No `unwrap()` /
  `expect()` on fallible external input in library code (tests are exempt).
- No commented-out code, no dead code left behind ("temp", "TODO remove").
- Comments explain *why*, not *what*. Do not add comments the code does not
  need.

## 5. Security principles

Full model: `docs/security-model.md`. Hard rules:

- **Never** log, print, commit, or embed API keys, tokens, passwords, or
  other secrets. Never store secrets in the repository or in plaintext files.
- Never expose secrets to webpage JavaScript, the DOM, extensions, or logs.
- Web content is **untrusted data, never instructions**. Page text must
  never be spliced into a context where it can be interpreted as commands
  to Jerry, the extension host, or the LLM system prompt without clear
  untrusted-content delimiters and tool-level gating.
- Jerry cannot take DESTRUCTIVE actions without explicit user confirmation;
  SENSITIVE actions are permission-aware; INTERACTION is controlled; READ is
  generally safe. The architecture must keep these tiers enforceable even
  before they are fully implemented.
- Browser and Jerry must remain isolated: a Jerry crash must not kill
  browsing; a page failure must not kill the browser; neither gets ambient
  filesystem or arbitrary network access.
- Extensions are untrusted and run sandboxed with capability grants.
- No `unsafe` to bypass a safety check.

## 6. Privacy principles

Full model: `docs/privacy-model.md`. Hard rules:

- **No telemetry, no analytics, no remote logging — ever — unless a human
  explicitly designs and approves it.** Do not add any outbound network
  call "just in case", for update checks, font fetching, DNS prefetch,
  crash reporting, or dependency convenience.
- All browser-originated network traffic must route through
  `halley-network` so it is auditable and blockable.
- Default configuration is privacy-preserving: telemetry off, tracker
  blocking on, Jerry off, destructive confirmation on (enforced by
  `Config::default()` — keep it that way).
- BYOK: user keys stay local, go to the user's chosen provider only, and
  are never sent anywhere else.
- Local-first storage; data is encrypted at rest in later milestones.
- Logging is local stderr only (`halley_common::logging`). Never log page
  content at default levels. Never log secrets.

## 7. Testing requirements

- Every behavioural change needs a test that would fail without the change.
- Unit tests live in the crate (`#[cfg(test)]`); cross-crate tests in
  `crates/<crate>/tests/`.
- Tests assert real behaviour. Never hardcode outputs to force a pass, never
  skip failing tests, never delete a failing test instead of fixing it.
- Default test suite performs **no network I/O**. Tests requiring network
  must be explicitly marked and excluded by default.
- Do not write tests for functionality that does not exist yet — write the
  functionality, then the test.
- Before reporting completion run, in order:
  `cargo fmt --all -- --check`,
  `cargo check --workspace --all-targets --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo test --workspace --all-features`
  (or `scripts/verify.ps1` / `scripts/verify.sh`).
  Ignored GUI e2e tests run only when explicitly requested:
  `cargo test -p halley-core --test e2e_smoke -- --ignored`.

## 8. Dependency rules

Before adding **any** dependency (Cargo, npm, action, tool):

1. Explain why it is required *now* (not later).
2. Verify it fits the architecture and license (compatible with Apache-2.0).
3. Prefer mature, Rust-native, widely-audited crates.
4. Do not add browser-engine crates, AI SDKs, WASM runtimes, or crypto
   libraries until the milestone actually needs them.
5. Never add a dependency that phones home, bundles telemetry, or pulls
   secrets-handling you did not ask for.
6. Pin GitHub Actions to reviewed versions; prefer official `actions/*`.

Current external dependencies (engine + session + Jerry BYOK transport;
see ADR-006, ADR-007, ADR-010):

| Crate | Why now | License |
| --- | --- | --- |
| `wry` 0.57 | Platform webview embedding (one child view per tab) | Apache-2.0 OR MIT |
| `tao` 0.37 | Native window + event loop paired with wry | Apache-2.0 |
| `raw-window-handle` 0.6 | Window-handle trait shared by wry/tao/core | MIT OR Apache-2.0 OR Zlib |
| `url` 2 | Address-bar / homepage / network-request URL parsing (core, common, privacy, network) | MIT OR Apache-2.0 |
| `serde` 1 | Session snapshot / tab JSON schema (`schema_version`) | MIT OR Apache-2.0 |
| `serde_json` 1 | Session snapshot serialization | MIT OR Apache-2.0 OR JSON |
| `cookie` 0.18 | `Set-Cookie` parsing for `halley-privacy::CookieStore` (no hand-rolled parser) | MIT OR Apache-2.0 |
| `ureq` 2 | BYOK LLM provider HTTPS in `halley-jerry` only (ADR-010; endpoint allowlisted; not used by page loads) | MIT OR Apache-2.0 |

Do not add further dependencies until the current milestone actually needs
them. Re-check AGENTS.md rules §8 before any new crate.

### Tab / session rules (ADR-007)

- Tab ids are stable `TabId`s, never strip indices.
- The tab strip is **never empty** after construction (last close opens
  `new_tab_url`).
- Engine pages are 1:1 with open tabs; destroy on close.
- Session snapshots carry `schema_version`; unknown versions are rejected.
- Popup/`window.open` is denied as a native window and becomes a Halley tab.
- `Stop` is `NotSupported` until the engine exposes `stop()` — do not fake
  a Stop button or success result.
- Keyboard shortcuts live in `assets/chrome.html` and are chrome-scoped
  when content focus steals platform accelerators — document limits, do
  not invent a fake native accelerator layer.

### Network / privacy rules (ADR-008, ADR-009)

- In-process Halley-originated HTTP goes through
  `halley_network::NetworkManager` only — no ad-hoc sockets, no new
  default-transport I/O, no telemetry “just in case”. **Exception:**
  BYOK LLM provider calls in `halley-jerry` are a separate allowlisted
  category (ADR-010) and do not use `NetworkManager`.
- Default `Transport` stays `NoTransport`; tests use `MockTransport`.
  Never add a real HTTP client without an AGENTS §8 justification and an
  ADR when the milestone needs it. `ureq` in `halley-jerry` is justified
  by ADR-010 (Prompt #5).
- No dependency edge from `halley-network` to `halley-privacy` (or the
  reverse). Shared types live in `halley-common`; `halley-core`
  composes policies.
- Cookie *policy* lives in `halley-privacy::CookieStore`; platform-webview
  page cookies remain engine-owned until an interception milestone —
  do not pretend the policy jar sees live page cookies.
- Private profiles: unique system-temp root, never under
  `Config.storage.profile_root`, full delete on `close()`.
- All path joins with untrusted names go through
  `sanitize_component` / `safe_join_component` — never raw `join`.
- Never log `Authorization`, cookie values, or secret query params;
  use `halley_network::redact` helpers. Errors must not embed secrets.
- TLS verification may only be disabled when `app.dev_mode` is true
  (`Config::validate` enforces this) — never in release defaults.

## 9. Documentation rules

- Code changes that alter behaviour, boundaries, or security posture must
  update the relevant doc in the same change.
- Architecture decisions get an ADR in `docs/adr/` (numbered, with Context /
  Decision / Reason / Consequences / Alternatives). Mark undecided things
  UNDECIDED — never invent technical details the PRD has not chosen.
- Status labels in docs: **IMPLEMENTED**, **PLANNED**, **NOT IMPLEMENTED**,
  **UNDECIDED**. Use them honestly; documentation-only protections are never
  IMPLEMENTED.
- Public APIs are documented in code; prose docs explain boundaries and
  intent, not line-by-line code.

## 10. Git rules

- Inspect `git status` and `git diff` before starting and after finishing.
- Never reset, force-push, delete, or overwrite existing work. Never revert
  changes you did not make. If unrelated changes exist, leave them alone.
- Commit only when the human explicitly asks. Never amend, rebase, or push
  unprompted.
- Small, focused commits with messages that describe the why. Never commit
  secrets, keys, `.env` files, or `target/`.
- Do not commit generated artifacts or editor droppings.

## 11. How agents must work

1. Read `AGENTS.md` (this file) first, every session.
2. Read the relevant docs/ADRs before touching a subsystem.
3. Inspect the repository before modifying it — preserve existing work.
4. Plan before large edits; keep changes within the current milestone.
5. Stay inside your lane: do not implement future milestones "while you're
   here" (no Jerry planner, adblock rules, extensions, LLM calls,
   network choke-point rewiring, etc., beyond the current milestone).
6. Verify with the commands in §7. Never report success on unverified work.
7. If blocked or ambiguous, ask — do not guess product behaviour.

## 12. How agents must report completion

Every task ends with a report containing exactly these sections:

```
## Completed
- What was implemented (bullet list, factual)

## Repository Structure
- Important files/directories added or changed

## Architecture
- How the change fits the architecture

## Tests
- Commands run and their actual results (pass/fail counts)

## Security
- Security-sensitive decisions and their justification

## Privacy
- Privacy-related decisions (especially any network I/O)

## Not Implemented
- Everything intentionally deferred

## Known Issues
- Unresolved problems, caveats, flaky spots

## Next Recommended Milestone
- Only the single next logical step; do not start it
```

Include a recommended commit message, but **never commit unless asked**.
Never claim a feature works without having run the verification commands.
