# ADR-010: Jerry LLM transport (BYOK egress)

- **Status:** IMPLEMENTED (transport allowlist + `ureq` client for provider
  calls only)
- **Date:** 2026-09-23

## Context

AGENTS.md / ADR-008 require in-process Halley-originated HTTP to funnel
through `halley-network::NetworkManager` so it is auditable and blockable.
Prompt #5 needs Jerry to call a **user-configured BYOK LLM provider**
(OpenAI-compatible, Anthropic, Gemini, or local endpoint) for chat
completions. That traffic is a distinct product category from browser
navigation: it carries API keys in headers, goes only to the endpoint the
user explicitly configured, and must not create a second unbounded socket
path inside Jerry.

`halley-jerry` may not depend on `halley-core` or `halley-network`
(ADR-002 dependency direction: `halley-core` sits above both; Jerry talks
to the browser only through MCP contracts). Routing LLM calls through
`NetworkManager` would invert the graph or force a dependency edge that
does not exist.

ADR-008 already anticipated a separate, explicitly-allowlisted Jerry LLM
category (`docs/architecture.md` § network boundaries).

## Decision

- **Category:** BYOK provider calls from `halley-jerry` are a **separate
  allowlisted egress category**, not browser page traffic and not
  `NetworkManager` traffic.
- **Client:** `halley-jerry::transport::UreqTransport` owns a small
  `ureq` 2.x client (TLS feature; no default cookie/redirect sprawl).
  Redirects are disabled (`redirects(0)`); timeouts are capped.
- **Allowlist:** every request URL must pass
  `halley_jerry::provider::validate_endpoint`:
  - scheme `https` only, except `http` when the host is loopback
    (`127.0.0.1`, `::1`, `localhost`) for local/Ollama-style endpoints;
  - no embedded userinfo;
  - no unexpected ports tricks beyond normal URL parsing;
  - path joining for dialect endpoints stays under the validated base.
- **Secrets:** API keys are sent only in request **headers**
  (`Authorization`, `x-api-key`, `x-goog-api-key` as appropriate for the
  dialect). Keys never appear in the URL, never in logs, never in chrome
  state JSON.
- **Transport trait:** `ProviderTransport` (`Send + Sync`) with
  `MockTransport` (tests; records calls, canned bodies, scripted stream
  deltas) and `UreqTransport` (production). The default suite never
  constructs `UreqTransport` against a non-loopback host; integration
  tests use `MockTransport`.
- **Streaming / cancel:** streaming uses a buffered body read with line-wise
  SSE parsing; cooperative cancel is an `AtomicBool` checked between
  deltas. Mid-body socket abort is **not** implemented (documented
  limitation).
- **No telemetry:** the transport performs no discovery, no update check,
  no analytics. One user-triggered send/test → one request to the
  configured endpoint.

## Reason

- Keeps AGENTS §8 honest: `ureq` is justified now because Prompt #5
  requires real BYOK completions; it is confined to `halley-jerry` and an
  allowlisted host list, not a general browser HTTP stack.
- Avoids a false dependency of Jerry on `halley-network` (which would also
  pull privacy/network policy into the agent crate).
- Header-only keys + endpoint validation + redaction match the security
  and privacy models (secrets never in URL or logs).

## Consequences

- LLM traffic is **not** visible to `NetworkManager` policy/adblock hooks
  yet; auditability is `UreqTransport` + `MockTransport` call recording +
  endpoint allowlist tests. A future ADR may bridge categories if the
  product needs one choke point for *all* egress including BYOK.
- `ProviderTransport` must be `Sync` so `JerryRuntime` can move into
  worker threads for streaming; this is a trait-bound choice, not an
  unsafe escape hatch.
- Custom base URLs that are plain `http` on non-loopback hosts are
  rejected (except when `app.dev_mode` policy is relaxed in a later
  milestone if product requires it — **not** open now).
- Default `Config` still has `jerry.enabled = false`; no automatic calls.

## Alternatives

- **Send BYOK calls through `halley-network::NetworkManager`:** rejected —
  would require `halley-jerry` → `halley-network` or `halley-core` glue
  that breaks ADR-002; also NetworkManager has no JSON/SSE client yet and
  the default transport is intentionally no-I/O.
- **`reqwest`:** rejected — heavier async runtime footprint for a single
  streaming path; `ureq` is enough for Prompt #5 scale.
- **Hand-rolled TCP/TLS:** rejected — reinventing TLS is unacceptable.
- **Allow arbitrary HTTP endpoints:** rejected — keys would leave the
  machine to attacker-controlled hosts if config were poisoned.

## Status notes

- **IMPLEMENTED:** allowlist, headers-not-query keys, Mock/Ureq split,
  redirect disable, redaction of provider error bodies, unit tests that
  never hit the network.
- **NOT IMPLEMENTED:** mid-body cancel (socket drop), HTTP/2 tuning,
  proxy configuration, routing through `NetworkManager` for a unified
  egress log.
