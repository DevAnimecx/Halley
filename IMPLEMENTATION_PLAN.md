# Halley Implementation Plan

Living plan for milestone work. Each prompt appends or updates its own
section. Status labels: **PLANNED**, **IN PROGRESS**, **IMPLEMENTED**,
**BLOCKED**.

---

## Prompt #1–#4 (previous)

IMPLEMENTED (see git history / final reports): workspace skeleton, engine
embedding (wry/tao multi-page), multi-tab session + chrome tab strip,
network pipeline (ADR-008), privacy profiles/cookies/storage (ADR-009).
All gates green as of Prompt #4.

---

## Prompt #5 — Jerry Intelligence Foundation

**Status:** IMPLEMENTED

Build the conversational intelligence layer (Jerry subsystem, BYOK
providers, context, privacy pipeline, structured responses, read-only tool
contracts, minimal chat UI). **No autonomous browser control.**

### Architecture discovered (reusable)

| System | Where | Reuse for Jerry |
| --- | --- | --- |
| Config (`AiConfig`, `JerryConfig`) | `halley-common::config` | Provider/model names + enable flags (no secrets) |
| Profile + storage paths | `halley-privacy::{BrowserProfile,StorageManager}` | Credential/conversation files under profile root via `safe_path` |
| Private profile temp delete | `BrowserProfile::close` / private root | Do not persist Jerry memory under private roots |
| Network redaction | `halley_network::redact` | Key/URL redaction in Jerry logs (same `REDACTED` token) |
| Event wake pattern | `AppEvent::EngineWake` + `EventLoopProxy` | Jerry async completions wake the same loop |
| Chrome IPC | `ChromeAction` / `chrome_state_script` | New Jerry actions + Jerry state push |
| Tab/session state | `Browser` / `BrowserSession` | `PageContextProvider` inputs (url/title/tab id) |
| Dependency graph ADR-002 | `docs/adr/ADR-002` | `core → jerry → {jerry-mcp, jerry-opt} → common` only |

### Architecture (this prompt)

```
chrome.html (chat UI)
    │ ChromeAction::Jerry*  /  __halleyUpdateJerry(state)
    ▼
halley-core::jerry_host          ── owns JerryHost, drains JerryEvent
    │ JerryRequest / BrowserPageContext
    ▼
halley-jerry::runtime            ── conversation, context budget, privacy
    │ ProviderTransport (trait)
    ├─► UreqTransport            ── real HTTPS to user-configured endpoint only
    └─► MockTransport            ── tests (no network)
    │
    ├─ uses halley-jerry-opt     ── token estimate + context prioritization
    └─ advertises tools from
         halley-jerry-mcp        ── READ-only contracts (no executor loop yet)
```

**Credential storage:** profile-scoped JSON under
`<profile>/profile/jerry-credentials.json` written only through
`StorageManager::safe_path`. Not `Config`. Not OS keychain
(**NOT IMPLEMENTED** — Prompt #4 provided isolation + path safety only;
documented in security model updates). Private profiles: memory-only
credentials (no disk write).

**LLM transport:** `halley-jerry` owns a small `ureq`-based transport for
BYOK egress (architecture allows Jerry LLM traffic as a separate
allowlisted category). Endpoint allowlist: https only; custom base URLs
must be https unless host is loopback. API keys only in headers, never
query strings, never logs. Real network I/O only when the user triggers
send/test with a configured provider (default suite stays offline).

**Chat UI:** Jerry toggle in chrome toolbar expands chrome height
(`set_layout` with `chrome_height + jerry_panel_height`). Panel is plain
chrome HTML matching existing CSS variables. State pushed as
`window.__halleyUpdateJerry({…})`.

**Context:** `PageContextProvider` builds `ContextItem`s with
`Provenance::{User,Browser,System,Webpage,…}`. Budget via
`halley-jerry-opt` (char-based estimate + priority order). Page **text**
extraction is **NOT IMPLEMENTED** (engine has no DOM read API) — only
url/title/tab metadata as browser context.

**Tools:** `halley-jerry-mcp` defines `getCurrentTab`,
`getCurrentPageMetadata`, `getSelectedText` as READ contracts with JSON
schemas. No tool execution loop (autonomous control is a later prompt).

### New modules

- `crates/halley-jerry/src/{error,provenance,conversation,context,response,credentials,transport,provider,runtime,privacy}.rs`
- `crates/halley-jerry-mcp/src/{tool,registry,tools}.rs`
- `crates/halley-jerry-opt/src/budget.rs`
- `crates/halley-core/src/jerry_host.rs` + chrome Jerry actions/state
- `docs/adr/ADR-010-jerry-llm-transport.md`
- `docs/jerry-architecture.md` (boundaries + limitations)

### Testing strategy

- Unit: conversation store, credential redaction, context budget,
  provider request builders (no HTTP), MCP tool registry, endpoint
  allowlist rejects, privacy pipeline strips forbidden fields.
- Integration: `MockTransport` records request; assert Authorization
  header present, body has provenance-framed context, no key in logs.
- Real network: **not** in default suite (AGENTS §7).

### Unresolved / limitations (honest)

- No OS keychain / DPAPI for keys yet.
- No full-page DOM text for context.
- No agent planner / tool execution / multi-step actions.
- Streaming: buffered SSE parse via ureq; cancellation via shared flag
  checked between chunks (no mid-body socket kill).
- Local provider (`http://127.0.0.1` / `localhost`) allowed for Ollama-style
  endpoints; loopback https not required.
