# Jerry architecture (Prompt #5 — Intelligence Foundation)

Status labels: **IMPLEMENTED**, **PLANNED**, **NOT IMPLEMENTED**,
**UNDECIDED**.

This document describes what Prompt #5 actually ships: a conversational
Jerry layer with BYOK providers, local memory, provenance-framed context,
and a minimal chrome chat panel. It does **not** implement autonomous
browser control.

## 1. Scope

| Area | Status |
| --- | --- |
| Conversation store (JSON, `schema_version`) | **IMPLEMENTED** |
| BYOK provider kinds + request builders (OpenAI / Groq / Anthropic / Gemini / local) | **IMPLEMENTED** |
| `ProviderTransport` + `MockTransport` / `UreqTransport` | **IMPLEMENTED** |
| Streaming completions + cooperative cancel | **IMPLEMENTED** (buffered SSE; no mid-body abort) |
| Credential store under profile (`jerry-credentials.json`) | **IMPLEMENTED** (not OS keychain) |
| Conversation persistence (`jerry-conversations.json`) | **IMPLEMENTED** (normal profiles; memory-only when private) |
| Page context (url / title / tab id / tab count) | **IMPLEMENTED** |
| Context token budget (`halley-jerry-opt`) | **IMPLEMENTED** (char-based estimate) |
| Provenance framing + privacy system prompt | **IMPLEMENTED** |
| Structured responses (`StructuredResponse`, stop reasons) | **IMPLEMENTED** |
| Read-only MCP tool **contracts** (`halley-jerry-mcp`) | **IMPLEMENTED** (schemas only) |
| Tool **execution** / planner / multi-step actions | **NOT IMPLEMENTED** |
| Chrome chat UI (toggle, messages, composer, provider/model/key) | **IMPLEMENTED** (HTML/JS in `assets/chrome.html`) |
| `JerryHost` in core + `AppEvent::JerryWake` | **IMPLEMENTED** |
| DOM / full page text extraction | **NOT IMPLEMENTED** (engine has no DOM read API) |
| OS keychain / DPAPI | **NOT IMPLEMENTED** |
| Process isolation for Jerry | **UNDECIDED** (ADR-005 thread interim) |

## 2. Dependency placement

```
halley-core
  ├── halley-jerry ──► halley-jerry-mcp
  │                └──► halley-jerry-opt
  ├── halley-engine
  ├── halley-network
  ├── halley-privacy
  └── halley-extensions
              │
              ▼
        halley-common
```

- Jerry crates never depend on `halley-core` or `halley-engine`.
- BYOK HTTPS uses `halley-jerry`'s `ureq` transport (ADR-010), not
  `halley-network` (dependency direction).

## 3. Control flow (chat turn)

1. User focuses chrome, toggles Jerry (`ChromeAction::JerryToggle`) or
   sends text (`JerrySend`).
2. `Browser::apply_chrome_action` classifies Jerry actions and emits
   `BrowserEvent::JerryAction` (no browser command).
3. `Application` drains events → `apply_jerry_events` →
   `JerryHost::handle_action` / `spawn_send`.
4. Worker thread owns `JerryRuntime` for the turn:
   - build `BrowserPageContext` from active tab;
   - assemble system + provenance-framed context + history under budget;
   - call `ProviderTransport` (stream or non-stream);
   - publish partial text + final `StructuredResponse` on a shared job slot.
5. `AppEvent::JerryWake` → main loop `jerry.poll()` → update
   `JerryChromeState` → `Browser::set_jerry_state` → relayout if panel
   open changed → `push_chrome`.

## 4. State pushed to UI

`JerryChromeState` (serialized into chrome state, never includes secrets):

- `open`, `enabled`, `provider`, `model`, `has_api_key`
- `status`: `disabled` | `idle` | `connecting` | `streaming` | …
- `messages`: role + content (+ streaming flag)
- `context_note`, `error` (user-visible, redacted)

API key **value** is never part of this struct. Entering a key posts
`jerrykey <secret>` once; chrome clears the input immediately.

## 5. Storage

| File | Path category | Notes |
| --- | --- | --- |
| `jerry-credentials.json` | `StoragePaths::Profile` via `safe_path` | schema_version; private profiles: memory only |
| `jerry-conversations.json` | `StoragePaths::Profile` via `safe_path` | active thread + history; private: not written |

Not under `Config`. Not in repo. Logging never prints key material.

## 6. Security boundaries (this prompt)

- Endpoint allowlist before any I/O (ADR-010).
- Keys in headers only; never URL, never chrome Debug, never logs.
- Page metadata is **untrusted data** framed with provenance labels; no
  DOM body is sent.
- No tool executor: model output is treated as assistant text only.
- Cooperative cancel only; no claim of hard network abort.

## 7. Testing

- Unit tests in `halley-jerry*` and `jerry_host` (no network).
- Cross-crate: `crates/halley-core/tests/prompt5_regression.rs`
  (MockEngine + MockTransport chat flow, chrome routing, height expand,
  key non-leak).
- Real provider I/O is manual / user-triggered only.

## 8. What is intentionally out of scope

Autonomous navigation, clicking, form fill, planner, multi-step tasks,
MCP tool execution loop, screenshots, DOM scraping, model routing across
providers beyond a single selected provider/model, process sandbox for
Jerry.
