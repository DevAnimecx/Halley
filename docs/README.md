# Halley Documentation

Documentation index for the Halley project.

| Document | Purpose |
| --- | --- |
| [architecture.md](architecture.md) | System overview, subsystems, dependency and process boundaries |
| [security-model.md](security-model.md) | Threat model, trust boundaries, secrets, Jerry safety (status-marked) |
| [privacy-model.md](privacy-model.md) | Intended privacy architecture (status-marked) |
| [jerry-architecture.md](jerry-architecture.md) | Prompt #5 Jerry chat layer: modules, control flow, limitations |
| [performance-baseline.md](performance-baseline.md) | Performance metrics and measurement rules (tab budgets + network/privacy suites; no browser I/O numbers claimed yet) |
| [adr/](adr/) | Architecture Decision Records (001–010; engine: ADR-006; tabs/sessions/popups: ADR-007; network pipeline: ADR-008; profiles/storage/cookies: ADR-009; Jerry LLM transport: ADR-010) |
| [PRD.md](PRD.md) | Product requirements document (scope source of truth) |

Status labels used throughout the docs:

- **IMPLEMENTED** — exists and works in this repository today.
- **PLANNED** — decided, not yet built.
- **NOT IMPLEMENTED** — decided as intent only; details may still change.
- **UNDECIDED** — no decision yet.
