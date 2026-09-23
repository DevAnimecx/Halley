# ADR-008: Network request pipeline and policy boundary

- Status: **IMPLEMENTED** (policy/pipeline foundation; no real I/O transport yet)

## Context

AGENTS.md requires all Halley-originated outbound traffic to funnel through
`halley-network` so it is auditable and blockable. Prompt #4 asks for a
centralized network stack: request model, per-request context, timeouts,
redirect rules, TLS policy, typed errors, and log redaction — without
introducing a real HTTP client (which would be premature and would create
unaudited I/O).

## Decision

- `NetworkManager` is the only entry point for Halley-originated requests:
  `decide()` (policy) → header assembly (UA, referrer hooks) → `Transport` →
  redirect loop (`RedirectTracker`) → typed `NetworkResponse`/`NetworkError`.
- Transport is an injectable trait. The **default is `NoTransport`**, which
  performs **no I/O** and returns `NetworkError::NoTransport`. Tests use
  `MockTransport` (scripted responses, no sockets). A real HTTP client is
  deferred to the milestone that needs it.
- Policy (`NetworkPolicy::evaluate`) returns
  `RequestDecision::{Allow, Block, Modify}`. Blocking and header
  modification are the composition points for future `halley-adblock`
  filtering — adblock is **not implemented** yet; the seam exists.
- Redirects: max hops from `Config::network.max_redirects` (default 10),
  HTTPS→HTTP downgrade blocked, non-http(s) targets rejected.
- TLS: `TlsPolicy` records intent (`verify_certificates`); disabling
  verification is only allowed when `app.dev_mode` is true (enforced by
  `Config::validate`). The platform webview/transport owns the actual
  handshake; Halley never ships a "verify off in release" path.
- Secrets: `redact.rs` redacts sensitive headers and secret query keys;
  `request_log_line` never embeds raw `Authorization`/`Cookie` values.

## Reason

Central choke point, honest no-I/O default, policy seam for adblock later,
no premature dependency on an HTTP client crate.

## Consequences

- Page loads initiated by the platform WebView still **bypass**
  `halley-network` until engine-level interception exists (known gap,
  documented in `docs/architecture.md`).
- Redirect method conversion (303/302 POST→GET) is simplified for the
  GET-only foundation; refine when a real client lands.
- `Config::network.user_agent` is a static string (no rotation/spoofing).

## Alternatives

- Real `reqwest`/`ureq` client now: rejected — needs milestone justification
  (AGENTS §8) and creates network I/O in the default suite (AGENTS §7).
- Policy living in `halley-privacy`: rejected — would create a sideways
  privacy→network or network→privacy edge (ADR-002). Shared `Origin` lives
  in `halley-common`; core composes.
