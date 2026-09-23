# Halley Privacy Model

**Status:** this is the intended privacy architecture. The foundation /
engine-spike / tabs / network-privacy-foundation milestones deliver only
the pieces explicitly marked **IMPLEMENTED** (safe defaults, local-only
logging, default homepage with no network, mock-engine tests with no
I/O, `halley-network` policy pipeline with default `NoTransport`,
`halley-privacy` profiles/cookies/storage policy — ADR-008 / ADR-009).
Navigating to an `http(s)` URL today still makes the **platform
webview** contact the network directly, and WebView page cookies are not
synced into `CookieStore` — those are documented gaps, not completed
protections. Never treat a documented intention as a working
protection.

Status labels: **IMPLEMENTED** · **PLANNED** · **NOT IMPLEMENTED** ·
**UNDECIDED**.

## 1. Principles

1. **No telemetry, no analytics, no remote logging — by default, forever**
   unless a human explicitly designs and approves a mechanism. Silence is
   the product.
2. **Privacy by architecture:** all browser egress funnels through
   `halley-network`, so "prove we send nothing" means auditing one crate.
3. **Local-first:** user data lives on the user's disk. No cloud sync.
4. **BYOK:** the user's LLM keys and prompts go to the user's chosen
   provider and nowhere else.
5. **Safe defaults:** a fresh profile is already private; hardening is
   opt-*out*, not opt-*in*.

## 2. Telemetry policy

| Item | Status |
| --- | --- |
| Telemetry/analytics/crash-reporting code | **IMPLEMENTED absence** — none exists; `PrivacyConfig.telemetry_enabled` defaults to `false` |
| Any future telemetry requires explicit design + human approval + ADR, and must be off by default with visible disclosure | **PLANNED** (policy) |
| Auto-update checks, remote font/config fetch, DNS prefetch for convenience | Forbidden unless explicitly designed; nothing of the sort exists today (**IMPLEMENTED absence**) |

## 3. Network policy

| Item | Status |
| --- | --- |
| Single egress choke point in `halley-network` (request model, policy, timeouts, redirects, TLS policy, redaction) | **PARTIALLY IMPLEMENTED** as in-process pipeline + typed errors (ADR-008); default `Transport` is **no I/O**; not wired into webview page loads |
| Platform webview performs its own sockets when loading `http(s)` pages | **IMPLEMENTED** as engine-spike reality (wry/WebView2/WebKitGTK; ADR-006) — known gap until traffic is routed through `halley-network` |
| Default homepage / new-tab URL is `about:blank` (no automatic network on start) | **IMPLEMENTED** (`Config::default().browser.homepage` and `new_tab_url`) |
| Default configuration blocks trackers | `PrivacyConfig.block_trackers = true` is **IMPLEMENTED** as a default; `NetworkPolicy` `Allow/Block/Modify` seam exists (**IMPLEMENTED**); real adblock rules (**NOT IMPLEMENTED**) |
| Static User-Agent (no rotation/spoofing); HTTPS→HTTP redirect downgrade blocked; max 10 redirects | **IMPLEMENTED** (`NetworkPolicy` / `UserAgentPolicy`) |
| Per-site permission for camera/mic/geo/etc., default-deny | **PARTIALLY IMPLEMENTED** — `PermissionPolicy` denies camera/mic/geo/clipboard-read, `ask` otherwise, per-`Origin` grants (**IMPLEMENTED** as policy); wry handler still **deny-all spike**; user prompt UI **NOT IMPLEMENTED** |
| Referrer policy (`StrictOriginWhenCrossOrigin` default; strips cross-origin/downgrade) | **IMPLEMENTED** as `ReferrerPolicy::referrer_value` computation; engine does not yet apply headers (**NOT IMPLEMENTED**) |
| User-visible connection/permission dashboard | **NOT IMPLEMENTED** |

## 4. DNS

| Item | Status |
| --- | --- |
| System-resolved DNS by default | **IMPLEMENTED** as `DnsPolicy::System` / `can_resolve_now` (no resolver I/O in this crate) |
| DNS-over-HTTPS (DoH) option with user-chosen resolver | **PLANNED**, details **UNDECIDED** (resolver defaults, fallback rules); `DnsPolicy::DohPending` is a **placeholder**, not a working DoH |
| No DNS prefetch/probing that leaks hostnames before navigation | **PLANNED** rule; nothing implemented beyond default no-I/O transport |

## 5. Cookies and site storage

| Item | Status |
| --- | --- |
| Third-party cookie blocking by default | **IMPLEMENTED** in `CookieStore` / `CookiePolicy` (`block_third_party = true`) for cookies set **through the policy jar** |
| `Set-Cookie` parsing via mature crate (`cookie`), SameSite/Secure/HttpOnly/expiry enforced in policy layer | **IMPLEMENTED** (`halley-privacy`) |
| Private browsing: separate in-memory jars, separate storage root under system temp, deleted on `close()` | **IMPLEMENTED** (`BrowserProfile`; tests: §50 isolation + cleanup) |
| Normal vs private jars never share files/cookies | **IMPLEMENTED** (profile tests + property tests) |
| Platform-webview **page** cookies synced into `CookieStore` | **NOT IMPLEMENTED** (ADR-009 ownership split — webview still owns live page cookies) |
| Per-site storage controls, easy site-data clearing | **PARTIALLY IMPLEMENTED** — `StorageManager::clear_site_component` / `clear_cache` / `clear_temp` for **policy-side** stores; no UI, no webview data wipe (**NOT IMPLEMENTED**) |
| Partitioned storage per top-level site (PSL-aware) | **PLANNED** — first/third party currently approximated via `site_label` (no Public Suffix List yet) |
| Encryption at rest for cookie/session files | **PLANNED** / key handling **UNDECIDED** |

## 6. WebRTC

| Item | Status |
| --- | --- |
| WebRTC local-only or disabled-by-default to prevent IP leaks | **PLANNED**, exact default **UNDECIDED** pending engine choice |
| Enforcement | **NOT IMPLEMENTED** — no WebRTC stack integrated |

## 7. Fingerprint resistance

| Item | Status |
| --- | --- |
| Reduce/normalize fingerprint surfaces (canvas, fonts, UA, timezone, hardware concurrency, WebGL) | **PLANNED** scope; approach **UNDECIDED** pending engine (`halley-engine`) capabilities |
| Resisting fingerprinting must not itself become a unique fingerprint | Explicit constraint for future work |
| Implementation | **NOT IMPLEMENTED** — `PrivacyConfig` does not even carry a fingerprint flag yet; adding flags before enforcement would fake progress |

## 8. Jerry's local memory

| Item | Status |
| --- | --- |
| Jerry's memory is local, stored separately from browser profile data | **IMPLEMENTED** for chat history (`jerry-conversations.json` under profile via `safe_path`; private profiles: memory-only) |
| User can inspect and delete Jerry memory at any time | **PARTIALLY IMPLEMENTED** — file is ordinary local JSON the user can delete; no in-app inspector yet (**NOT IMPLEMENTED**) |
| Page content remembered in memory | **N/A for Prompt #5** — only url/title/tab metadata + user/assistant text are stored (no DOM body) |
| Long-term agent memory / embeddings | **NOT IMPLEMENTED** (future milestone) |

## 9. LLM providers (BYOK)

Jerry sends task-relevant context to the provider the user configured.

| Item | Status |
| --- | --- |
| BYOK: keys entered by user; profile-scoped `jerry-credentials.json` via `safe_path` (**IMPLEMENTED**); OS secure storage (mechanism **UNDECIDED**, see security model) **NOT IMPLEMENTED**; never in plaintext `Config` | **PARTIALLY IMPLEMENTED** |
| Requests go only to the user's configured provider endpoint; endpoint allowlisting enforced by `halley_jerry::provider::validate_endpoint` (ADR-010) | **IMPLEMENTED** for `halley-jerry` transport |
| No provider calls until the user configures one and enables Jerry (`JerryConfig.enabled=false` by default); send requires an explicit chrome action | Defaults **IMPLEMENTED**; runtime gate **IMPLEMENTED** via `check_ready()` (missing key/provider → error, no request) |
| Separate egress category (not `halley-network` / no telemetry) | **IMPLEMENTED** (ADR-010) |

### 9.1 What data is sent to the provider

**IMPLEMENTED** for Prompt #5 (chat context only — no autonomous tasks yet):

- Conversation history for the active thread + the new user message.
- Browser page context: active tab id, URL, title, open tab count
  (metadata only — **no DOM body, no cookies, no history**).
- Provenance-framed context block + privacy system prompt suffix.
- Never: other providers' API keys, browsing history, cookies, local
  files, or content from unrelated tabs.
- The user is informed what leaves the machine as part of task execution
  (surfacing design **NOT IMPLEMENTED** beyond the context note in the
  panel).

**PLANNED** for autonomous tasks: page excerpts/tool results with
untrusted framing; richer disclosure UI.

What each external provider does with data is governed by that provider's
policy — Halley can only control what it sends. This tradeoff is inherent
to BYOK and must stay visible in product copy.

## 10. Local data

| Item | Status |
| --- | --- |
| Profile data confined to a per-profile directory | **IMPLEMENTED** (`StorageManager` categories: `profile\|cache\|cookies\|session\|logs\|temp`; root from `Config.storage.profile_root`) |
| Path traversal from untrusted names cannot escape profile root | **IMPLEMENTED** (`sanitize_component` / `safe_join_component` + property tests) |
| Private profile never written under the normal profile root | **IMPLEMENTED** (`ProfileError::NotAllowed` if nested; unique `halley-private-{pid}-{seq}` temp root) |
| Encryption at rest for sensitive stores; key handling needs its own ADR | **PLANNED** / key handling **UNDECIDED** |
| Export only via explicit user action; deletion actually deletes | **PLANNED** requirement; private root delete on `close()` is **IMPLEMENTED** |

## 11. Logging

| Item | Status |
| --- | --- |
| Logs go to local stderr only; never remote | **IMPLEMENTED** (`halley_common::logging`) |
| Never log secrets, API keys, or page content at default levels | **IMPLEMENTED** as documented rule of the logging API (nothing sensitive is logged today because nothing is logged) |
| Log-level control via `HALLEY_LOG` env var | **IMPLEMENTED** |
| Production log redaction layer | **PARTIALLY IMPLEMENTED** — `halley_network::redact` strips sensitive headers/URL secrets before any request log line; general-purpose redacting logger for Jerry still **NOT IMPLEMENTED** |

## 12. Privacy manifest

**PLANNED:** a machine-readable `PRIVACY.md`/manifest shipped with releases
enumerating every network endpoint Halley can contact, every permission
requested, and every local store written — generated from code where
possible so it cannot drift from reality. Not created yet because there are
no endpoints, permissions, or stores to enumerate; creating one now would
be speculation.

## 13. Verification

- Today: default test suite still performs **no network I/O** (mock
  engine + URL parsing only). CI enforces fmt/check/clippy/test with
  `--all-features` on Linux and Windows.
- Known gap: once a user loads an external `http(s)` URL, egress is
  whatever the platform webview does — **not** yet auditable solely by
  reading `halley-network`.
- Future: tests that fail if code outside `halley-network` opens a
  connection; a manifest check; documented manual audit procedure. These
  are **PLANNED** ideas, not existing tests.
