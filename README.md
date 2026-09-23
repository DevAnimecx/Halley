# Halley

**Privacy-first browser written in Rust, with a built-in autonomous AI
browser agent called Jerry.**

> **Current status: multi-tab session + network/privacy foundation + Jerry chat (BYOK).**
> This repository compiles a Cargo workspace, opens a native window with a
> tab strip and address bar (back / forward / reload / navigate), renders
> each tab as its own platform web view (wry; ADR-006), supports typed
> browser commands (open/close/activate/reorder/reopen tabs, session
> snapshots in memory), has an in-process **network policy pipeline**
> (ADR-008; default transport does no I/O) and **privacy profiles**
> (normal vs private, storage isolation, cookie/permission/referrer
> policy — ADR-009), and ships a **Jerry chat panel** (BYOK providers,
> streaming completions, local credential/conversation files, read-only
> tool **contracts** — ADR-010 / `docs/jerry-architecture.md`). It does
> **not** yet route page loads through the choke point, block ads, store
> history to disk, run an agent planner, or execute browser tools. See
> [Current status](#current-status) for an honest inventory.

## Vision

Halley aims to be a lightweight, auditable browser where:

- **Privacy is structural, not a setting** — no telemetry or analytics by
  default, all network egress flows through one auditable choke point,
  network-level tracker blocking, fingerprint resistance, local-first
  encrypted storage.
- **The user holds the keys** — Jerry uses your own (bring-your-own-key)
  LLM provider credentials, stored locally, sent only to the provider you
  choose.
- **Jerry does the boring parts for you** — an autonomous agent that plans
  and executes multi-step browser tasks (research, form-filling, repeated
  workflows) through a narrow MCP tool protocol, with permission tiers and
  explicit confirmation before anything destructive.
- **Hostile by default** — web pages are untrusted data, never
  instructions; extensions run sandboxed; the agent cannot exfiltrate
  secrets it never sees.
- **Reproducible and open** — Apache-2.0 licensed, reproducible
  cross-platform builds, architecture decisions recorded in public ADRs.

## Architecture

Modular Rust workspace; one crate per subsystem, strict one-directional
dependency graph:

```
halley-core            application orchestration & browser state (+ `halley` binary)
  ├─ halley-engine     rendering-engine boundary (wry / WebView2 / WebKitGTK; ADR-006)
  ├─ halley-network    all outbound traffic → adblock → privacy policy
  ├─ halley-privacy    privacy policies & enforcement points
  ├─ halley-jerry      the AI agent ──► halley-jerry-mcp (tool protocol)
  │                                        └─ halley-jerry-opt (routing)
  └─ halley-extensions WASM extension sandbox
            │
            ▼
      halley-common     shared errors, config, logging
```

Jerry talks to the browser **only** through MCP tools — never into core
internals. Full details: [`docs/architecture.md`](docs/architecture.md),
ADRs in [`docs/adr/`](docs/adr/) (engine choice: ADR-006), security and
privacy models in [`docs/`](docs/).

## Current status

**Implemented today (multi-tab session + chrome + network/privacy foundation + Jerry chat):**

- Virtual Cargo workspace with 10 crates, all compiling
- Shared error strategy, configuration types with privacy-preserving
  defaults + `Config::validate()` (TLS-off only in dev mode), local-only
  logging (`halley-common`); shared `Origin` / `site_label` primitives
- Engine abstraction (`BrowserEngine` multi-page trait) with a **native
  wry/tao backend** and a **test mock** behind feature flags (ADR-006)
- Launchable `halley` binary: native window, trusted chrome (tab strip +
  address bar, back / forward / reload), one content web view per tab,
  URL normalization (URL vs search), native history; opens the normal
  profile at startup
- Typed `BrowserCommand` / `BrowserEvent`, `BrowserSession` lifecycle +
  JSON snapshots (`schema_version`), bounded closed-tab stack (ADR-007)
- Chrome keyboard shortcuts (Ctrl+T/W/Tab, Ctrl+L, Alt+←/→, F5, Ctrl+1..9)
  with documented content-focus limits
- **Network pipeline foundation (ADR-008):** `NetworkManager` +
  `NetworkPolicy` (allow/block/modify), centralized timeouts, redirect
  limit + HTTPS→HTTP downgrade block, UA/TLS policy, typed errors, log
  redaction; default `NoTransport` (no sockets), `MockTransport` for tests
- **Privacy foundation (ADR-009):** normal vs private `BrowserProfile`
  (private temp root deleted on close), categorized traversal-safe
  storage, in-memory `CookieStore` (third-party block, private
  ephemeral), `PermissionPolicy` default-deny, `ReferrerPolicy`
- **Jerry chat foundation (ADR-010, Prompt #5):** `halley-jerry`
  conversation store, BYOK providers (OpenAI / Groq / Anthropic / Gemini /
  local), header-only API keys, endpoint allowlist, `MockTransport` /
  `UreqTransport` (streaming + cooperative cancel), provenance-framed
  context + token budget (`halley-jerry-opt`), read-only MCP tool
  contracts (`halley-jerry-mcp`), `JerryHost` + chrome chat panel
  (toggle / send / stop / provider / model / key), profile-scoped
  `jerry-credentials.json` + `jerry-conversations.json`
- Documentation suite: architecture, security model, privacy model,
  jerry-architecture, ADRs 001–010, performance baseline notes
- Unit + integration tests, property-style tab + path/cookie invariant
  tests, ignored GUI e2e smoke tests, GitHub Actions CI on Linux + Windows
- Workspace-wide `unsafe` denial in Halley crates

**Not implemented (everything else):** history/cookies/downloads UI,
session auto-restore from disk, routing **page** loads through
`halley-network` (webview still opens sockets directly), syncing WebView
page cookies into `CookieStore`, tracker-block rules (adblock),
fingerprinting protections, DoH, encrypted storage, Stop (wry has no
`stop()` — command returns typed `NotSupported`), Jerry planner, MCP tool
**execution**, OS keychain for API keys, DOM/page-body context extraction,
agent long-term memory, model routing across providers, extensions
runtime, `ui/` frontend, performance tuning.

Never confuse docs with demos: a described protection is **not** a working
protection. Docs label everything **IMPLEMENTED / PLANNED / NOT
IMPLEMENTED / UNDECIDED**.

**Known privacy gap:** when the user navigates to an `http(s)` URL, the
platform webview performs network I/O directly — page traffic is not yet
routed through `halley-network`. Default homepage is `about:blank` (no
network). `CookieStore` does not yet see live WebView page cookies.

## Repository structure

```
├── Cargo.toml            virtual workspace root
├── AGENTS.md             engineering constitution for AI coding agents
├── crates/
│   ├── halley-common/    errors, config, logging (bottom of the graph)
│   ├── halley-core/      orchestration + `halley` binary + tests
│   ├── halley-engine/    BrowserEngine trait, wry backend, mock
│   ├── halley-network/   network choke point (pipeline + mock transport)
│   ├── halley-adblock/   filtering (empty)
│   ├── halley-privacy/   privacy policies, profiles, cookie store
│   ├── halley-jerry/     Jerry runtime, providers, privacy, conversation
│   ├── halley-jerry-mcp/ tool contracts (READ-only; no executor yet)
│   ├── halley-jerry-opt/ token estimate + context budget
│   └── halley-extensions/ WASM sandbox (empty)
├── docs/                 architecture, security, privacy, jerry-architecture, adr/, performance-baseline
├── tests/                cross-crate test docs/fixtures
├── benches/              benchmark docs (no benchmarks yet)
├── scripts/              local verification scripts (mirror CI)
├── ui/                   frontend (none yet)
├── assets/               chrome.html (embedded chrome UI)
└── .github/workflows/    CI
```

## Development setup

Prerequisites:

- [rustup](https://rustup.rs/) with the stable toolchain
  (`rust-toolchain.toml` pins stable + `rustfmt` + `clippy`; workspace
  `rust-version` is 1.85).
- Windows: WebView2 Runtime (preinstalled on current Windows 11).
- Linux (for building the default `native` feature):
  `libwebkit2gtk-4.1-dev` and `libgtk-3-dev` (and related build deps);
  CI installs the same packages.

```sh
git clone <repo-url>
cd Halley
cargo check --workspace --all-targets --all-features   # compile everything
cargo test --workspace --all-features                  # run all tests
cargo run -p halley-core --bin halley                  # open the browser
cargo run -p halley-core --bin halley https://example.com   # optional URL/search
```

Or run the full local gate (same as CI):

```sh
./scripts/verify.sh         # POSIX
.\scripts\verify.ps1        # Windows
```

GUI end-to-end smoke tests are ignored by default (they need a display):

```sh
cargo test -p halley-core --test e2e_smoke -- --ignored
```

## Testing

```sh
cargo test --workspace --all-features
```

- Unit tests live in each crate; integration tests in `crates/<crate>/tests/`
- Tests assert real behaviour only; no network I/O in the default suite
  (mock engine + `about:blank` / local URL parsing only)
- Ignored GUI e2e tests: `cargo test -p halley-core --test e2e_smoke -- --ignored`
- CI runs `cargo fmt --all -- --check`, `cargo check --workspace
  --all-targets --all-features`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, and `cargo test --workspace
  --all-features` on Linux and Windows for every push and pull request

See [`tests/README.md`](tests/README.md) and
[`benches/README.md`](benches/README.md).

## Roadmap

Ordered milestones; each stays behind the AGENTS.md rules (no fake
features, verify before claiming done):

1. **Genesis / foundation** — workspace, docs, tests, CI. *(done)*
2. **Skeleton app** — minimal runnable binary proving packaging. *(done
   as part of the engine spike: `halley` binary boots a window.)*
3. **Engine spike / first browser** *(done)* — ADR-006 engine,
   `halley-engine` wry adapter, address-bar chrome, back/forward/reload,
   single-tab rendering.
4. **Tabs & sessions** *(done)* — multi-tab strip, typed commands,
   session lifecycle + snapshot foundation (ADR-007).
5. **Network + privacy foundation** *(done as foundation only)* —
   ADR-008 pipeline/policy (no-I/O default transport), ADR-009 profiles,
   storage isolation, cookie/permission/referrer policy. **Remaining:**
   rewire page loads behind the choke point, real transport, adblock
   rules, WebView cookie sync.
6. **Privacy hardening** — blocking defaults, storage partitioning,
   fingerprint work (per privacy-model).
7. **History UI / richer navigation** — history UI, bookmarks, session
   auto-restore (beyond the in-memory snapshot foundation).
8. **Jerry foundation** — MCP tool protocol, safety tiers, confirmation
   UX, BYOK key storage.
9. **Jerry autonomy** — planner, memory, routing, token optimization.
10. **Extensions sandbox** — WASM runtime with capability grants.
11. **Release engineering** — reproducible builds, cross-platform
    packages, privacy manifest.

Performance work and benchmarks attach to whichever milestone creates the
measurable subsystem; see [`docs/performance-baseline.md`](docs/performance-baseline.md).

## Contributing / AI agents

Read [`AGENTS.md`](AGENTS.md) **before** any change. It is the binding
engineering constitution: dependency rules, security/privacy hard lines,
verification commands, and the required completion-report format.
Architectural changes require a new ADR in `docs/adr/`.

## License

Licensed under the [Apache License 2.0](LICENSE).
