# ADR-003: Security Boundaries

- **Status:** Accepted
- **Date:** 2026-09-23

## Context

Halley combines hostile inputs (arbitrary web pages), an LLM agent that
can act on the user's behalf, third-party extensions, and user secrets
(BYOK API keys). The security model (`docs/security-model.md`) needs
structural boundaries so future implementation cannot accidentally collapse
them.

## Decision

Establish five non-negotiable trust boundaries in the architecture:

1. **Web content → browser:** pages are hostile data. Page JS must never
   receive secrets; page failures must be contained at the engine seam
   (`halley-engine`).
2. **Web content / LLM output → Jerry:** webpage text and model output are
   untrusted *data*, never instructions. All authority flows through MCP
   tools with server-side tier checks (READ / INTERACTION / SENSITIVE /
   DESTRUCTIVE); DESTRUCTIVE requires explicit user confirmation
   (`require_confirmation_for_destructive`, default `true`).
3. **Secrets → everything else:** API keys live (future) in OS-level
   secure storage only; they never enter logs, config files, the DOM,
   extension APIs, or any prompt context. Today: no key storage exists at
   all, and `AiConfig` has no secret fields.
4. **Extensions → host:** WASM extensions run capability-scoped with no
   ambient filesystem/network/secret access.
5. **Repository policy:** workspace-wide `unsafe_code = "deny"`; no
   telemetry/analytics code; CI denies clippy warnings.

## Reason

- Boundaries that exist *before* features land are cheap; boundaries
  retrofitted after secrets have leaked are not.
- Prompt injection cannot be solved by prompt wording alone — gating must
  live in code the model cannot talk to, hence tool-tier checks.
- Keeping keys out of Jerry's context makes full LLM jailbreaks harmless
  with respect to secrets.

## Consequences

- Every future MCP tool must carry a declared tier at definition time; an
  unclassified tool is a bug.
- Secure-storage choice (keychain/DPAPI/keyring) remains **UNDECIDED**
  and requires its own ADR when key entry ships.
- Process/thread model for Jerry isolation is settled in ADR-005, not
  here.

## Alternatives considered

- **Trusting the system prompt to keep the agent in line:** rejected —
  prompt injection is unsolvable at that layer alone.
- **Storing keys in config with file permissions:** rejected — plaintext
  secrets on disk violate the security model.
- **Allowing extensions full DOM + network like legacy browsers:**
  rejected — capability grants are the PRD's requirement.
