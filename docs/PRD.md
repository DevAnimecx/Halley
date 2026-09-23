# Product Requirements Document (PRD)
## **Halley** — Privacy-First Rust Browser with **Jerry** AI Agent

| Field | Value |
|-------|-------|
| **Product Name** | Halley |
| **AI Agent Name** | Jerry |
| **Version** | 1.0 (PRD) |
| **Date** | 23 September 2026 |
| **Status** | Draft → Review |
| **Engine** | Obscura v0.2.1 (Apache-2.0) |
| **Protocol** | MCP 2026-07-28 (stateless) |
| **License** | Apache-2.0 (open source) |

---

## 1. Executive Summary

**Halley** is a single-binary, 100% Rust web browser that is the most privacy-proven browser in existence, with a built-in agentic AI called **Jerry** that can perform any browser task on the user's behalf.

**Core thesis**: No browser has combined *provability privacy* + *ultra-lightweight performance* + *full agentic AI* + *BYOK token efficiency* in a single product. Halley does all four.

| Pillar | Claim | Proof Mechanism |
|--------|-------|-----------------|
| Privacy | Zero telemetry, fingerprint-resistant, verifiable | Reproducible builds + `about:privacy` manifest + source audit |
| Performance | <30 MB/tab, <300 ms cold start, 5.7 μs ad block | Benchmarks in CI, published |
| AI Power | Jerry does every browser task, chats, researches, automates | 150+ MCP tools, multi-step planning, model routing |
| Cost | 80–90% cheaper than naive AI browsers | 8-layer token optimization engine |

---

## 2. Target Users & Use Cases

### 2.1 Primary Persona

| Attribute | Description |
|-----------|-------------|
| **Who** | Privacy-conscious developer / power user |
| **Pain** | Chrome/Firefox are bloated, track them, and their AI features are cloud-locked |
| **Need** | A fast, private browser where *their* AI (their key, their model) can do anything a human could do in the browser |
| **Willing to pay** | Free (open source). Saves money via BYOK + token optimization |

### 2.2 Secondary Personas

| Persona | Use Case |
|---------|----------|
| **Privacy advocate** | Needs provable no-telemetry, fingerprint resistance, for activism / journalism |
| **AI researcher** | Wants a local, auditable agent that can browse, extract, and reason without cloud dependency |
| **Cost-sensitive user** | Uses cheap models (DeepSeek, local Llama) and wants the browser to be token-efficient so their bill stays low |
| **Automation developer** | Uses Halley as a lightweight CDP/MCP target for scripts (Obscura's native CDP) |

### 2.3 Key Use Cases (Jerry)

| # | User Says | Jerry Does |
|---|-----------|------------|
| 1 | "Summarize this page" | Extracts text → sends to LLM → returns 3-bullet summary |
| 2 | "Find the cheapest 1TB SSD on Amazon" | Navigates, searches, extracts prices, compares, reports |
| 3 | "Fill this form with my details" | Reads form fields → asks for missing data → fills + submits |
| 4 | "Compare these 3 tabs" | Reads all 3 snapshots → diffs → structured comparison |
| 5 | "Book a flight Delhi→Mumbai, Friday, cheapest" | Multi-step: search → filter → select → fill → confirm → submit |
| 6 | "Alert me when this price drops below ₹500" | Sets monitor → polls periodically → notifies |
| 7 | "What's new in the Rust release notes?" | Navigates → extracts → summarizes → highlights changes |
| 8 | "Translate this page to Hindi" | Extracts text → LLM translates → renders in Jerry panel |
| 9 | "Run this JS in the console" | `evaluate_js` tool → returns result |
| 10 | "Remember that I prefer economy seats" | Writes to local preference store → uses in future bookings |

---

## 3. Product Requirements

### 3.1 Functional Requirements

#### FR-1: Browser Core

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1.1 | Navigate to any URL (http/https) | P0 |
| FR-1.2 | Multi-tab support (unlimited tabs) | P0 |
| FR-1.3 | Back / Forward / Reload | P0 |
| FR-1.4 | Bookmark management (local, encrypted) | P0 |
| FR-1.5 | Password manager (AES-256-GCM, OS keychain) | P0 |
| FR-1.6 | History (local, deletable, no sync) | P0 |
| FR-1.7 | Downloads manager | P1 |
| FR-1.8 | Zoom in/out, text size adjustment | P1 |
| FR-1.9 | Fullscreen, picture-in-picture | P1 |
| FR-1.10 | Developer tools (console, network, elements) | P2 |
| FR-1.11 | PDF export (via Obscura native rendering) | P1 |
| FR-1.12 | Screenshot / screencast (via Obscura) | P1 |

#### FR-2: Ad/Tracker Blocking

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-2.1 | Network-level ad blocking (before data arrives) | P0 |
| FR-2.2 | Cosmetic (CSS) ad hiding | P0 |
| FR-2.3 | DNS-level tracker blocking | P1 |
| FR-2.4 | User-manageable filter lists (EasyList, EasyPrivacy, NoBid, custom) | P0 |
| FR-2.5 | Per-site allow/block toggle | P1 |
| FR-2.6 | Block element picker (right-click → "Block this element") | P1 |
| FR-2.7 | Real-time block count in address bar | P2 |

#### FR-3: Privacy

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.1 | Zero telemetry (no analytics, no crash reporting by default) | P0 |
| FR-3.2 | DNS-over-HTTPS only (no plain DNS) | P0 |
| FR-3.3 | TLS fingerprint randomization (JA3/JA4 rotation per session) | P0 |
| FR-3.4 | Canvas noise (per-session consistent) | P0 |
| FR-3.5 | WebGL spoofing (report generic GPU) | P0 |
| FR-3.6 | `navigator.*` uniformization | P0 |
| FR-3.7 | Font enumeration blocking (fixed list) | P0 |
| FR-3.8 | WebRTC disabled by default | P0 |
| FR-3.9 | Cookie partitioning (first-party isolation) | P0 |
| FR-3.10 | Storage partitioning per origin | P0 |
| FR-3.11 | HSTS preload (built-in list) | P0 |
| FR-3.12 | Certificate Transparency enforcement | P1 |
| FR-3.13 | Process isolation (each site in own process) | P0 |
| FR-3.14 | `about:privacy` page (machine-readable manifest) | P0 |
| FR-3.15 | Reproducible builds (same source → same binary) | P0 |
| FR-3.16 | No account, no sync, no cloud dependency | P0 |
| FR-3.17 | Request timing jitter (50–200 ms random) | P2 |
| FR-3.18 | HTTP/2 fingerprint randomization | P2 |

#### FR-4: Jerry AI Agent

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-4.1 | Chat interface (sidebar panel) | P0 |
| FR-4.2 | BYOK: user provides API key (OpenRouter, OpenAI, Anthropic, Google, DeepSeek, Ollama, custom) | P0 |
| FR-4.3 | Model routing (auto-select model by task complexity) | P0 |
| FR-4.4 | Token optimization (8-layer: caching, structured state, routing, snapshots, progressive retrieval, output constraints, semantic cache, loop bounding) | P0 |
| FR-4.5 | Navigate to URL on behalf of user | P0 |
| FR-4.6 | Click elements | P0 |
| FR-4.7 | Fill form fields | P0 |
| FR-4.8 | Scroll, hover, keyboard input | P0 |
| FR-4.9 | Extract text/tables/data from page | P0 |
| FR-4.10 | Multi-step task planning (chain-of-thought) | P0 |
| FR-4.11 | User confirmation for destructive actions (submit, purchase, delete) | P0 |
| FR-4.12 | Local memory (preferences, past actions) via SQLite | P1 |
| FR-4.13 | Session summary (compressed context, not raw transcript) | P0 |
| FR-4.14 | Token budget system (user-set cost ceiling per task) | P1 |
| FR-4.15 | Append-only audit log (all Jerry actions) | P0 |
| FR-4.16 | Local LLM support (Ollama / llama.cpp / mistral-rs) for offline use | P1 |
| FR-4.17 | Streaming responses (token-by-token) | P0 |
| FR-4.18 | Multi-tab awareness (Jerry can read/compare multiple tabs) | P1 |
| FR-4.19 | Page monitoring / alerts (price drops, content changes) | P2 |
| FR-4.20 | Code execution in page context (`evaluate_js`) | P1 |
| FR-4.21 | "One Neuron" learning (single perceptron learns from user interactions) | P2 |
| FR-4.22 | MCP 2026-07-28 stateless protocol (no session state) | P0 |

#### FR-5: Extensions

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-5.1 | WASM-based extension sandbox (WASI) | P2 |
| FR-5.2 | Extension manifest (JSON) | P2 |
| FR-5.3 | Resource limits (64 MB memory, CPU fuel, message rate) | P2 |
| FR-5.4 | Extension store (local, community-curated) | P3 |

#### FR-6: Performance

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-6.1 | Cold start < 300 ms | P0 |
| FR-6.2 | Memory per tab < 30 MB | P0 |
| FR-6.3 | Ad block check < 10 μs | P0 |
| FR-6.4 | Page load (JS-heavy) < 200 ms (LCP) | P1 |
| FR-6.5 | Binary size < 80 MB | P0 |
| FR-6.6 | Lazy tab rendering (pause JS/rAF in background) | P0 |
| FR-6.7 | Arena allocation for DOM (bumpalo) | P1 |
| FR-6.8 | mimalloc global allocator | P1 |
| FR-6.9 | HTTP/3 (QUIC) support | P1 |
| FR-6.10 | Preconnect + speculative DNS resolution | P2 |
| FR-6.11 | GPU compositing (wgpu: Vulkan/Metal/D3D12) | P1 |
| FR-6.12 | Zero-copy networking (tokio + hyper + bytes) | P1 |

### 3.2 Non-Functional Requirements

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-1 | Memory safety | 100% Rust, no unsafe blocks in core (audited) |
| NFR-2 | Cross-platform | Windows 10+, macOS 12+, Linux (glibc 2.31+) |
| NFR-3 | Single binary | No runtime dependencies beyond OS libraries |
| NFR-4 | Offline capability | Browser works fully offline. Jerry works with local LLM. |
| NFR-5 | Accessibility | ARIA compliance, keyboard navigation, screen reader support |
| NFR-6 | i18n | English (launch), Hindi (v1.1) |
| NFR-7 | Crash resilience | Tab crash ≠ browser crash. Agent crash ≠ browser crash. |
| NFR-8 | Update mechanism | Manual download (no auto-update by default) |
| NFR-9 | Disk footprint | < 200 MB installed (binary + default filter lists) |

---

## 4. Technical Architecture

### 4.1 System Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  HALLEY (single Rust binary, <80 MB)                                       │
│                                                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  UI LAYER (Tao/Winit + wgpu)                                          │ │
│  │  ┌──────────┐ ┌──────────┐ ┌─────────────┐ ┌───────────────────────┐  │ │
│  │  │ Tab Strip │ │ Address  │ │ Jerry Panel │ │ Privacy Dashboard     │  │ │
│  │  │          │ │ Bar      │ │ (chat +     │ │ (about:privacy)       │  │ │
│  │  │          │ │          │ │  audit log) │ │                       │  │ │
│  │  └──────────┘ └──────────┘ └─────────────┘ └───────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  ENGINE LAYER (Obscura v0.2.1)                                        │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────────────┐  │ │
│  │  │ V8 (via  │ │ html5ever│ │ Layout   │ │ wgpu Compositor          │  │ │
│  │  │ deno_core)│ │ DOM      │ │ (flex,   │ │ (Vulkan/Metal/D3D12)    │  │ │
│  │  │          │ │          │ │  grid,   │ │                          │  │ │
│  │  │          │ │          │ │  block)  │ │                          │  │ │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────────────────┘  │ │
│  │  CDP Server (Puppeteer/Playwright compatible)                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  NETWORK LAYER (tokio + hyper + rustls + quinn)                       │ │
│  │  ┌────────┐ ┌──────────────┐ ┌──────────┐ ┌────────────────────────┐  │ │
│  │  │ DoH    │ │ TLS Fingerprint│ │ HTTP/3  │ │ Request Jitter         │  │ │
│  │  │(hickory)│ │ Randomization │ │ (QUIC)  │ │ + CT Enforcement       │  │ │
│  │  └────────┘ └──────────────┘ └──────────┘ └────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  AD/TRACKER BLOCKER (adblock-rust 0.13 + FlatBuffers)                 │ │
│  │  100K+ rules │ 5.7 μs/req │ 75% less memory │ Zero-copy              │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  JERRY AI AGENT (isolated thread)                                      │ │
│  │  ┌──────────────────────────────────────────────────────────────────┐  │ │
│  │  │  LLM Client (BYOK)                                              │  │ │
│  │  │  • Router (task → model)                                        │  │ │
│  │  │  • Token Optimizer (8-layer)                                    │  │ │
│  │  │  • Streaming (SSE)                                              │  │ │
│  │  │  • Providers: OpenRouter/OpenAI/Anthropic/Google/DeepSeek/Local │  │ │
│  │  └──────────────────────────────────────────────────────────────────┘  │ │
│  │  ┌──────────────────────────────────────────────────────────────────┐  │ │
│  │  │  Agent Core                                                     │  │ │
│  │  │  • Planner (CoT) → Tool Caller → State Manager                 │  │ │
│  │  │  • MCP 2026-07-28 (stateless) client                           │  │ │
│  │  │  • Safety: confirmations, audit log, turn limits                │  │ │
│  │  └──────────────────────────────────────────────────────────────────┘  │ │
│  │  ┌──────────────────────────────────────────────────────────────────┐  │ │
│  │  │  Memory                                                         │  │ │
│  │  │  • Session Summary (compressed)                                 │  │ │
│  │  │  • Preference Store (SQLite)                                    │  │ │
│  │  │  • Neuron (single perceptron, 4 KB)                             │  │ │
│  │  └──────────────────────────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  EXTENSIONS (wasmtime + WASI sandbox)                                  │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
         │
         │  HTTPS (user's API key, direct to provider)
         ▼
┌──────────────────┐
│  User's LLM      │
│  Provider         │
│  (no middleman)   │
└──────────────────┘
```

### 4.2 Process Model

| Process | Contents | Isolation |
|---------|----------|-----------|
| **Main** | UI, tab strip, address bar, Jerry panel, settings | — |
| **Render × N** | One per site (Obscura V8 isolate + layout + paint) | Site isolation |
| **Network** | tokio runtime, DoH, TLS, adblock | Shared (no per-site state) |
| **Jerry** | LLM client, planner, memory, audit log | Separate thread (crash-safe) |
| **Extensions** | WASM instances | WASI sandbox (no FS, no net) |

### 4.3 Key Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Engine | **Obscura v0.2.1** | 30 MB/tab, V8, CDP+MCP native, Rust, Apache-2.0, 25K stars |
| Windowing | **Tao** (Winit-based) | Cross-platform, Rust-native, used by Tauri |
| GPU | **wgpu** | Vulkan/Metal/D3D12, Rust-native |
| Ad blocking | **adblock-rust 0.13** | 5.7 μs, FlatBuffers, Brave-proven |
| Network | **tokio + hyper + rustls + quinn** | Async, zero-copy, HTTP/3 |
| AI protocol | **MCP 2026-07-28 (stateless)** | No session state, cacheable, scalable |
| Local LLM | **mistral-rs** (optional) | Pure Rust, Metal/CUDA, no FFI |
| Extensions | **wasmtime + WASI** | Sandboxed, no native code |
| Allocator | **mimalloc** | 10–15% faster than system allocator |
| DOM arena | **bumpalo** | 30–50% fewer heap allocations |
| Storage | **SQLite (rusqlite)** | Jerry memory, bookmarks, history |
| Key storage | **keyring** (OS keychain) | AES-256-GCM, never plaintext |

---

## 5. Jerry AI — Detailed Specification

### 5.1 BYOK Configuration

```json
{
  "provider": "openrouter",
  "api_key": "<stored_in_os_keychain>",
  "default_model": "deepseek/deepseek-chat",
  "routing": {
    "trivial": "deepseek/deepseek-chat",
    "moderate": "anthropic/claude-haiku-4",
    "complex": "anthropic/claude-sonnet-4"
  },
  "budget": {
    "max_input_tokens": 8000,
    "max_output_tokens": 2000,
    "max_turns_per_task": 10,
    "cost_ceiling_per_task_usd": 0.05
  },
  "local_fallback": {
    "enabled": false,
    "endpoint": "http://localhost:11434",
    "model": "llama3.2:3b"
  }
}
```

### 5.2 Token Optimization Layers

| Layer | Mechanism | Expected Saving |
|-------|-----------|-----------------|
| 1. Prompt Caching | System prompt + tool defs cached at provider (KV cache) | 70–90% on repeated prefix |
| 2. Structured State | Compact state object replaces growing transcript | 40–60% |
| 3. Model Routing | Trivial→cheap, Complex→frontier | 50–80% per query |
| 4. Token-Efficient Snapshots | 2 KB page map vs 50 KB HTML | 60–80% on page context |
| 5. Progressive Retrieval | Fetch snippet only when needed | 30–50% |
| 6. Output Constraints | `max_tokens` + JSON schema | 20–40% on output |
| 7. Semantic Caching | Similar queries → cached response (local) | 30–50% hit rate |
| 8. Loop Bounding | Hard cap on iterations + min improvement | Prevents 50× blowup |

### 5.3 MCP Tool Surface (150+ Commands)

**Navigation**: `navigate`, `go_back`, `go_forward`, `reload`
**Understanding**: `snapshot`, `screenshot`, `extract_text`, `extract_table`, `get_meta`, `get_title`
**Interaction**: `click`, `fill`, `select`, `scroll`, `hover`, `keyboard`, `upload`, `drag`
**Forms**: `form_submit`, `form_reset`, `form_validate`
**Tabs**: `tab_new`, `tab_close`, `tab_switch`, `tab_list`, `tab_read`
**Network**: `network_log`, `fetch`, `wait_for`, `wait_for_url`, `wait_for_selector`
**Data**: `evaluate_js`, `get_storage`, `set_storage`, `get_cookies`, `search`
**Agent**: `memory_read`, `memory_write`, `memory_delete`, `ask_user`, `notify_user`, `set_monitor`, `cancel_monitor`
**System**: `get_page_info`, `get_performance`, `get_blocked_count`, `get_privacy_status`

### 5.4 Safety Model

| Rule | Implementation |
|------|---------------|
| **No autonomous destructive actions** | `form_submit`, purchase, delete → always `ask_user` first |
| **Turn limit** | Max 10 tool calls per task (configurable) |
| **Cost ceiling** | Stop if estimated cost > user-set limit |
| **No network from Jerry** | Jerry can only talk to browser's MCP server (localhost) |
| **Audit log** | Every action logged: timestamp, tool, params, result (append-only SQLite) |
| **Process isolation** | Jerry in separate thread; crash doesn't kill browser |
| **No data exfiltration** | Jerry cannot make arbitrary HTTP calls (only browser's `fetch` tool) |

### 5.5 The "One Neuron" Learning System

```rust
pub struct Neuron {
    pub weights: [f32; 64],  // 64 hash-bucketed features
    pub bias: f32,
    pub lr: f32,             // 0.005
}

// Trained on: (query_hash, position, dwell_time, clicked, hour, day)
// Output: relevance score for result ranking
// Size: 260 bytes on disk
// Updates: after each search interaction
// Uses: re-rank local suggestions, pre-fetch, suppress low-relevance
```

---

## 6. Privacy Specification (The "Proven" Claim)

### 6.1 Privacy Manifest (`about:privacy`)

```json
{
  "product": "Halley",
  "version": "1.0.0",
  "telemetry": "none",
  "network_calls_on_cold_start": 0,
  "network_calls_allowed": [
    "page_content (user-navigated)",
    "user_llm_provider (user-initiated, user's key)"
  ],
  "data_stored_locally": [
    "bookmarks (encrypted)",
    "passwords (AES-256-GCM, key in OS keychain)",
    "history (plaintext, user-deletable)",
    "jerry_memory (SQLite, user-deletable)",
    "jerry_audit_log (append-only SQLite)",
    "neuron_weights (260 bytes)",
    "adblock_rules (FlatBuffers)"
  ],
  "data_sent_remotely": [
    "user_llm_prompts → user's chosen provider (direct, no middleman)"
  ],
  "fingerprinting_resistance": {
    "canvas": "per-session noise",
    "webgl": "spoofed to generic GPU",
    "fonts": "fixed list (12 common fonts)",
    "navigator": "uniformized (4 cores, 8GB, 'Win32')",
    "audio": "disabled",
    "tls_ja3": "randomized per session",
    "http2": "randomized SETTINGS order"
  },
  "dns": "DoH only (Cloudflare/NextDNS, user-selectable)",
  "webrtc": "disabled",
  "cookies": "partitioned per origin",
  "storage": "partitioned per origin",
  "hsts": "preload list enforced",
  "certificate_transparency": "enforced",
  "reproducible_build": true,
  "build_hash": "sha256:abc123...",
  "source_url": "https://github.com/halley-browser/halley"
}
```

### 6.2 Verification Methods

| Claim | How to Verify |
|-------|---------------|
| No telemetry | `strace`/`lsof` on cold start → 0 network calls |
| Reproducible build | `cargo build --release` twice → identical SHA-256 |
| No closed-source | `cargo tree` → all deps are open source |
| Fingerprint resistance | Run on [browserleaks.com](https://browserleaks.com) → uniform results |
| Privacy manifest | `about:privacy` → machine-readable JSON |

---

## 7. Performance Benchmarks (Targets)

| Metric | Chrome | Firefox | Brave | **Halley Target** |
|--------|--------|---------|-------|-------------------|
| Cold start | 3.0 s | 2.0 s | 2.5 s | **<300 ms** |
| Memory (1 tab) | 120 MB | 80 MB | 120 MB | **<30 MB** |
| Memory (10 tabs) | 2.0 GB | 1.2 GB | 1.5 GB | **<300 MB** |
| Page load (JS-heavy, LCP) | 500 ms | 400 ms | 450 ms | **<200 ms** |
| Ad block per request | N/A | ~50 μs | 5.7 μs | **5.7 μs** |
| Binary size | 300+ MB | 100+ MB | 300+ MB | **<80 MB** |
| Jerry response (local 3B) | N/A | N/A | N/A | **<2 s** |
| Jerry response (API) | N/A | N/A | N/A | **<1 s** |
| Jerry cost (simple task) | $0.02–$0.05 | N/A | N/A | **$0.001–$0.005** |

---

## 8. Platform & Distribution

| Platform | Support | Package Format |
|----------|---------|---------------|
| Windows 10/11 (x64) | P0 | `.exe` single binary + `.msi` installer |
| macOS 12+ (Apple Silicon + Intel) | P0 | `.dmg` (universal binary) |
| Linux (glibc 2.31+) | P0 | `.AppImage`, `.deb`, `.rpm`, bare binary |

**Distribution model**: Open source (Apache-2.0). Download from GitHub Releases. No account, no auto-update, no telemetry. Optional GPG-signed releases.

---

## 9. Roadmap & Milestones

| Phase | Duration | Deliverable | Exit Criteria |
|-------|----------|-------------|---------------|
| **Phase 0: Skeleton** | Week 1–2 | Tao window + Obscura embed + navigation + tabs | Can open a URL, render it, switch tabs |
| **Phase 1: Network + Ad Block** | Week 3–4 | DoH, rustls, HTTP/3, adblock-rust | Ads blocked, DoH active, HTTPS works |
| **Phase 2: Privacy Stack** | Week 5–6 | Fingerprinting, partitioning, `about:privacy` | Passes browserleaks with uniform results |
| **Phase 3: Jerry v1** | Week 7–8 | BYOK + MCP tools + chat + navigate/click/fill | User can say "go to X and click Y" |
| **Phase 4: Jerry v2** | Week 9 | Token optimizer + model routing + structured state | 80%+ cost reduction vs. naive |
| **Phase 5: Jerry v3** | Week 10 | Memory + multi-step planning + audit log | Can do 5+ step tasks with confirmation |
| **Phase 6: Extensions + Perf** | Week 11–12 | WASM sandbox + mimalloc + arena + lazy tabs | <30 MB/tab, <300 ms cold start |
| **Phase 7: Hardening** | Week 13–14 | Security audit + reproducible builds + CI | Zero critical findings, reproducible hash |
| **v1.0 Release** | **Week 14** | **Public release** | All P0 requirements met |

### Post-1.0

| Version | Features |
|---------|----------|
| 1.1 | Hindi UI, WASM extensions, local LLM (mistral-rs), Neuron learning |
| 1.2 | Page monitoring/alerts, WebMCP support, multi-tab Jerry |
| 2.0 | Custom layout engine (if Obscura's renderer matures), full extension ecosystem |

---

## 10. Success Metrics

| Metric | Target (6 months post-launch) |
|--------|-------------------------------|
| GitHub stars | 5,000+ |
| Monthly active users | 10,000+ |
| Crash rate | < 0.1% sessions |
| Median cold start | < 300 ms |
| Median memory (5 tabs) | < 150 MB |
| Jerry task success rate | > 85% (on benchmark suite) |
| Jerry avg cost per task | < $0.01 |
| Privacy audit findings (critical) | 0 |
| User-reported privacy incidents | 0 |

---

## 11. Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Obscura web compat gaps (CSS/JS) | Medium | High | Track Obscura releases; WebKit fallback (wry) for problem sites |
| MCP spec changes (post 2026-07-28) | Low | Medium | Abstract MCP layer; follow Linux Foundation roadmap |
| LLM provider API changes | Medium | Low | Provider abstraction layer; BYOK means user adapts |
| Performance regression | Medium | Medium | CI benchmarks on every PR; performance budget in code review |
| Security vulnerability in V8 | Low | High | V8 is well-audited; update cadence; sandbox isolation |
| Scope creep (feature bloat) | High | Medium | Strict P0/P1/P2 gating; "does this serve privacy + speed + Jerry?" test |
| Single-maintainer risk | Medium | High | Open source; document architecture; seek contributors |

---

## 12. Competitive Positioning

| Feature | Chrome+Gemini | Firefox | Brave+Leo | Zeed | **Halley+Jerry** |
|---------|:---:|:---:|:---:|:---:|:---:|
| 100% Rust engine | ✗ | ✗ | ✗ | ✗ | **✓** |
| <30 MB/tab | ✗ | ✗ | ✗ | ✗ | **✓** |
| <300 ms cold start | ✗ | ✗ | ✗ | ✗ | **✓** |
| Zero telemetry (proven) | ✗ | Partial | Partial | ✗ | **✓** |
| Reproducible builds | ✗ | ✗ | ✗ | ✗ | **✓** |
| BYOK (any model) | ✗ | ✗ | Partial | ✓ | **✓** |
| Token optimization (8-layer) | ✗ | ✗ | ✗ | Partial | **✓** |
| Model routing (auto) | ✗ | ✗ | ✗ | ✗ | **✓** |
| Local LLM (offline) | ✗ | ✗ | ✗ | ✗ | **✓** |
| Full agentic (150+ tools) | Partial | ✗ | ✗ | Partial | **✓** |
| Built-in ad block (5.7 μs) | ✗ | ✗ (ext) | ✓ | ✗ | **✓** |
| WASM extensions | ✗ | ✗ | ✗ | ✗ | **✓** |
| Single binary <80 MB | ✗ | ✗ | ✗ | ✗ | **✓** |
| Open source (Apache-2.0) | ✗ | ✓ | ✓ | ✓ | **✓** |

---

## 13. File Structure (Monorepo)

```
halley/
├── Cargo.toml              # workspace
├── crates/
│   ├── halley-core/        # main binary, UI, tab management
│   ├── halley-engine/      # Obscura wrapper, CDP client
│   ├── halley-network/     # tokio, rustls, quinn, DoH, jitter
│   ├── halley-adblock/     # adblock-rust wrapper, rule management
│   ├── halley-privacy/     # fingerprinting, partitioning, manifest
│   ├── halley-jerry/       # AI agent (LLM client, planner, memory)
│   ├── halley-jerry-mcp/   # MCP 2026-07-28 server (browser side)
│   ├── halley-jerry-opt/   # token optimization (caching, routing)
│   ├── halley-extensions/  # wasmtime host, WASI sandbox
│   └── halley-common/      # shared types, error handling, config
├── ui/                     # Rust UI code (Tao + wgpu)
├── assets/                 # icons, default filter lists
├── docs/                   # this PRD, architecture docs, ADRs
├── .github/workflows/      # CI: build, test, benchmark, reproducibility
└── benches/                # performance benchmarks
```

---

## 14. Definition of Done (v1.0)

- [ ] All P0 functional requirements implemented and tested
- [ ] All P0 non-functional requirements met
- [ ] CI green: build + test + clippy + benchmarks + reproducibility check
- [ ] `about:privacy` page live and accurate
- [ ] Jerry can complete all 10 use cases in §2.3
- [ ] Token optimization achieves ≥70% saving vs. baseline (measured)
- [ ] Zero critical security findings in internal audit
- [ ] Binary <80 MB, cold start <300 ms, memory <30 MB/tab (verified)
- [ ] Documentation: README, architecture guide, Jerry user guide
- [ ] GitHub repo public, Apache-2.0, issue tracker active
- [ ] First blog post / HN post ready

---

## 15. Open Questions

| # | Question | Owner | Deadline |
|---|----------|-------|----------|
| 1 | Obscura rendering fidelity — is it sufficient for all P0 sites? | Engine team | Week 2 |
| 2 | Which LLM provider is the default recommendation for Indian users (cost)? | Jerry team | Week 6 |
| 3 | Should Jerry support voice input/output at v1.0 or v1.1? | Product | Week 4 |
| 4 | WebMCP support — wait for W3C finalization or implement early? | Jerry team | Week 10 |
| 5 | Extension format: WASM only or also allow JS (QuickJS)? | Extensions team | Week 11 |
| 6 | Should Halley support profile switching (work/personal)? | Product | Week 8 |

---

*End of PRD. Version 1.0 — 23 September 2026.*
