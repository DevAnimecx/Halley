# Halley Architecture

Status of this document: the boundaries described here are **PLANNED** except
where explicitly marked **IMPLEMENTED** (workspace layout, dependency graph,
`halley-common` primitives, the `halley-engine` wry/tao multi-page backend +
`Browser` orchestration with multi-tab sessions/commands, a launchable
`halley` binary with tab-strip chrome, the `halley-network`
policy/pipeline foundation (ADR-008, default no-I/O transport),
`halley-privacy` profile/storage/cookie policy with normal vs private
isolation (ADR-009), and the Prompt #5 Jerry chat layer — `halley-jerry*`,
`JerryHost`, BYOK transport (ADR-010), read-only tool contracts — are in
the repository today). Nothing described here should be read as a
feature-complete browser — see the repository README for current status.

## 1. Overview

Halley is a privacy-first browser written in Rust with a built-in
autonomous AI browser agent, Jerry. The system has two personas of code:

- **The browser** — rendering, tabs, networking, privacy enforcement,
  extensions. Serves the human user.
- **Jerry** — an LLM-driven agent that can observe and operate the browser
  on the user's behalf, through a narrow tool protocol, under safety rules.

They are peers connected by an explicit protocol, not one embedded inside
the other. This is the single most important structural decision in the
system (see `docs/adr/ADR-005-jerry-isolation.md`).

## 2. Core subsystems

| Crate | Responsibility |
| --- | --- |
| `halley-common` | Shared error strategy, configuration types, local logging. No business logic. |
| `halley-core` | Application orchestration and browser-level state (sessions, lifecycle). |
| `halley-engine` | Abstraction over the rendering engine; contains engine failures. |
| `halley-network` | All outbound network traffic; DNS policy; the audit choke point. |
| `halley-adblock` | Ad/tracker classification behind the network pipeline. |
| `halley-privacy` | Privacy policy definitions and enforcement points. |
| `halley-jerry` | Jerry: planning, task execution, local memory, safety gating. |
| `halley-jerry-mcp` | MCP tool protocol: the only Jerry↔browser interface. |
| `halley-jerry-opt` | Token optimization and model routing for Jerry's LLM calls. |
| `halley-extensions` | Sandboxed WASM extension host with capability grants. |

`ui/` (undecided toolkit) and `assets/` sit outside the crates.

## 3. Dependency boundaries

One-directional graph, enforced by Cargo and re-checked in review
(**IMPLEMENTED** as workspace structure; edges accrue as milestones land):

```
                        ┌─────────────┐
                        │ halley-core │            top: orchestration only
                        └──────┬──────┘
       ┌───────────┬──────────┼───────────┬──────────────┐
       ▼           ▼          ▼           ▼              ▼
 halley-engine halley-network halley-privacy halley-jerry halley-extensions
                    │                      │        │            │
                    ▼                      │   ┌────┴─────┐      │
              halley-adblock               │   ▼          ▼      │
                                            │ jerry-mcp jerry-opt │
                                            └────────┬────────────┘
                                                     ▼
                                               halley-common     bottom
```

Hard rules:

1. Arrows point downward only. No cycles, ever. Changing this requires an ADR.
2. `halley-common` depends on nothing in this repository.
3. Jerry crates never depend on `halley-core` or `halley-engine`. They reach
   the browser only through `halley-jerry-mcp` types and messages.
4. `halley-adblock` is depended on by `halley-network`, never the reverse,
   and is never reachable from rendering.
5. Cross-boundary data flows through plain, documented types — no shared
   mutable globals.

## 4. Browser engine boundary

`halley-engine` is the seam between Halley and the rendering engine
(**IMPLEMENTED** as a multi-page trait + two backends). Halley core code
never speaks an engine's native API; it speaks Halley-owned types
(`BrowserEngine`, `ChromeState`, `ChromeTabState`, `EngineEvent`,
`EngineMessage`, `EngineError`, `PageId`). The concrete engine for this
milestone is **wry + WebView2/WebKitGTK with a tao event loop** (ADR-006);
the boundary from ADR-004 still holds:

- Swapping or upgrading the engine touches only this crate plus ADR-006.
- Engine crashes/failures are caught at this seam so one bad page cannot
  unwind through the rest of the process (full isolation **NOT
  IMPLEMENTED** — errors surface as `EngineError`; process-level
  containment is future work).
- Feature split: `native` (default) is the wry backend; `test-engine` is
  an in-process mock with no window/network for integration tests.
- **Multi-page (IMPLEMENTED):** `create_page` / `destroy_page` /
  `set_page_visible` with `PageId`; inactive pages stay alive hidden
  (ADR-007). New-window requests are denied as native windows and
  reported to core, which opens a Halley tab.

Core (`halley-core`) owns the tao window/event loop and the multi-tab
`Browser` + `BrowserSession` + `TabManager` state machine (**IMPLEMENTED**
in memory; snapshots serialize with `schema_version` but do not yet
auto-restore from disk). The engine owns the chrome web view plus one
content web view per tab. Chrome UI is a trusted, embedded HTML strip
(`assets/chrome.html`) talking to Rust over wry IPC — not page content.
Typed `BrowserCommand` / `BrowserEvent` cover tab lifecycle, navigation,
and stop (`NotSupported` on wry — ADR-007).

## 5. Network boundary

All Halley-originated (in-process, non-page) outbound traffic is designed
to flow through `halley-network` (**PARTIALLY IMPLEMENTED** — ADR-008):
`NetworkManager` → `NetworkPolicy::evaluate` → `Transport`. The default
transport is `NoTransport` (**no sockets in the default suite**); tests
use `MockTransport`. A real HTTP client is **NOT IMPLEMENTED**. Redirect
limit, HTTPS→HTTP downgrade block, UA policy, TLS policy (verify on;
off only with `app.dev_mode` via `Config::validate`), and log redaction
are **IMPLEMENTED** as pipeline code.

- Page loads initiated by the **platform webview** (wry/WebView2/WebKitGTK)
  still open sockets directly when navigating to `http(s)` — that bypass
  is a known, documented gap until engine-level interception lands, not a
  design goal. Default homepage is `about:blank` (no network).
- `NetworkPolicy` returns `Allow | Block | Modify` — the composition point
  for future `halley-adblock` (consulting adblock from network is
  **PLANNED**; no edge wired yet; no real filtering rules exist).
- `halley-privacy` policies are composed by `halley-core` (shared
  `Origin`/`site_label` live in `halley-common`); there is **no**
  `network → privacy` dependency edge (ADR-002).
- Jerry's *tool-driven* navigation will reuse this pipeline; Jerry never
  gets a private socket. Jerry's *LLM* traffic (BYOK provider calls) is a
  separate, explicitly-allowlisted category governed by `halley-jerry`/
  `halley-jerry-opt` — it goes only to the user-configured provider
  endpoint (**NOT IMPLEMENTED**).

## 6. Privacy boundary

`halley-privacy` owns policy: what is allowed to leave, what is
partitioned, what is randomized. Enforcement points sit at the boundaries
already listed — network (headers, DNS, WebRTC), storage (cookies,
localStorage), engine (fingerprint surfaces), and Jerry (what page data
may enter prompts). Policy is data, enforcement is at the seam.

**IMPLEMENTED (policy layer only, ADR-009):** normal vs private
`BrowserProfile` with separate storage roots (private under system temp,
deleted on close), categorized storage paths with traversal-safe
joins, in-memory `CookieStore` (third-party block, SameSite/Secure/
HttpOnly/expiry, private ephemeral), `PermissionPolicy` default-deny for
camera/mic/geo/clipboard-read, `ReferrerPolicy`. **NOT IMPLEMENTED:**
syncing platform-webview page cookies into `CookieStore`, UI prompts for
permission grants, engine application of referrer headers, fingerprint
resistance, WebRTC policy. See `docs/privacy-model.md`.

## 7. Jerry boundary

Jerry is a separate failure and trust domain (**PARTIALLY IMPLEMENTED**,
ADR-005; chat layer in Prompt #5):

- **Failure isolation:** chat completions run on a `JerryHost` worker
  thread; a failed/panicking completion must not stop browsing
  (**IMPLEMENTED** for async chat jobs — still one OS process overall).
- **Authority:** today Jerry has **no tool executor** — it can only
  converse and read url/title/tab metadata (**IMPLEMENTED** as
  restriction). When tools land, authority is only what
  `halley-jerry-mcp` schemas allow. No ambient filesystem, no ambient
  sockets; LLM egress only to the allowlisted BYOK endpoint (ADR-010).
- **Safety tiers:** every tool is classified READ → INTERACTION →
  SENSITIVE → DESTRUCTIVE. Higher tiers require user gating
  (confirmation/permissions) before execution. Tier classification is a
  property of the tool schema, not something the LLM can talk its way out
  of (**NOT IMPLEMENTED** — `RiskLevel` exists on tool **contracts** only).

See also `docs/jerry-architecture.md`.

## 8. MCP boundary

`halley-jerry-mcp` defines tool schemas and the request/response contract.
It is the *only* intended interface between Jerry and the browser when
tools exist. Properties the boundary must hold:

- Tools are an explicit allowlist. Anything not in the schema is
  unreachable. (**IMPLEMENTED** as `ToolRegistry` contracts; **no
  executor** in Prompt #5.)
- Page-derived content crossing this boundary arrives as *data* with
  untrusted framing — never as pre-digested instructions.
  (**IMPLEMENTED** for the provenance context path in `halley-jerry`;
  page body text is not sent yet.)
- The browser side re-validates every tool call; it does not trust Jerry's
  process to have already checked permissions. (**NOT IMPLEMENTED** — no
  tool calls are accepted yet.)

## 9. Storage boundary

Local-first: profile data, cookies, history, and Jerry memory live on disk
in per-profile directories (**PARTIALLY IMPLEMENTED** — ADR-009 +
Prompt #5 Jerry files):

- `halley-privacy::BrowserProfile` with `Normal` (under
  `Config.storage.profile_root`) and `Private` (unique system-temp root,
  full delete on close; refused if nested under the normal root).
- Categories `profile | cache | cookies | session | logs | temp` are
  separated; path joins reject `..`, `/`, `\`, `:`, NUL, controls, and
  encoded traversal (**IMPLEMENTED** + property tests).
- Cookie *files* for page content are not yet written/synced from the
  platform webview (**NOT IMPLEMENTED** — see ADR-009 ownership split).
- Rules decided for when the rest lands:
  - Encryption at rest for sensitive stores (key management **UNDECIDED** —
    needs its own ADR).
  - No sync/telemetry channels. Export is explicit, user-initiated.
  - Jerry's memory is stored separately from browser data so it can be
    inspected/deleted independently. (**IMPLEMENTED** for chat —
    `jerry-credentials.json` / `jerry-conversations.json` under profile
    `safe_path`; independent inspection UI still **NOT IMPLEMENTED**.)

## 10. Extension boundary

`halley-extensions` hosts WASM extensions with **no ambient authority**.
Capabilities (specific host APIs, network domains) are granted explicitly
and are visible to the user (**PLANNED**). An extension cannot: read the
filesystem, open arbitrary sockets, read Jerry's memory, read API keys, or
escape its memory limits. Extensions and Jerry are mutually untrusted.

## 11. Security boundaries

Summarized from `docs/security-model.md` (read that document for the full
threat model):

| Boundary | Untrusted side | Trusted side | Mechanism |
| --- | --- | --- | --- |
| Rendering | Web page | Browser core | Engine sandbox + `halley-engine` seam |
| Network | Remote server | `halley-network` | All egress funneled and filtered here |
| Agent | LLM output / page text | Safety gate in Jerry/MCP | Tool allowlist + tier gating + confirmation |
| Extensions | Third-party WASM | `halley-extensions` host | Capability grants, resource limits |
| Secrets | Everything else | Keystore (future) | Keys never enter DOM/Jerry/logs |

Principle repeated everywhere: **web content is untrusted data, never
instructions.**

## 12. Future process model

**UNDECIDED** in specifics (ADR required before choosing), decided in
intent:

- Browser UI/core must survive Jerry dying → Jerry should run with at
  least crash containment (separate thread today; separate process if/when
  the threat model demands it).
- One tab/page failure must not take down the browser → engine-side
  containment per `halley-engine`.
- Extensions are sandboxed runtimes, already isolated from both browser
  core and Jerry.
- IPC between domains uses explicit, versioned messages — the same
  discipline as the MCP boundary — not shared mutable memory.

Candidate models (thread-per-domain, multiprocess like modern browsers,
WASM-component sandbox for Jerry) are compared in ADR-005; the final pick
waits for the engine decision (ADR-004) and a dedicated process-model ADR.
