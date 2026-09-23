# Halley Performance Baseline

**Status:** partial measurement **IMPLEMENTED** for mock tab
orchestration and for the Prompt #4 no-I/O network/privacy suites;
native WebView memory/startup remain **PLANNED**. Numbers without machine
context are not baselines.

## Principles

- Attach benchmarks to the milestone that creates the measurable
  subsystem; do not invent metrics for code that does not exist.
- Prefer wall-clock / allocation counters that reflect user-visible
  behaviour (cold start, navigation latency) over microbenchmarks that
  cannot regress in practice.
- Never claim a performance property without a reproducible command.

## Metrics

| Metric | Target subsystem | Milestone (status) |
| --- | --- | --- |
| Process cold start to first window painted | `halley-core` + wry/tao | Engine spike (window exists; **not measured**) |
| Address-bar Enter → navigation started | `halley-engine` + webview | Engine spike (**not measured**) |
| Mock open/close/activate for 1/5/10/20 tabs | `halley-core` `Browser` + `MockEngine` | Tabs & sessions (**IMPLEMENTED** in `tests/tab_perf.rs`, generous CI budgets) |
| Per-tab WebView memory (PRD `<30 MB/tab`) | wry/WebView2 | Tabs & sessions (**NOT MEASURED** — requires manual Task Manager / RSS sampling on a real window) |
| Policy decide → mock transport round-trip (no sockets) | `halley-network` | Network+privacy foundation (**IMPLEMENTED** as unit tests; not a formal bench) |
| Path-safety / cookie property iterations (500/200 LCG cases) | `halley-privacy` | Network+privacy foundation (**IMPLEMENTED** in `tests/property_isolation.rs`) |
| Page load time (fixture site / local server) | network + engine | Network pipeline (**NOT IMPLEMENTED** — no real transport) |
| Filter-list decision time per request | `halley-adblock` | Adblock milestone (**NOT IMPLEMENTED**) |
| Binary size (release, per platform) | packaging | Release engineering (**PLANNED**) |

## Reproducible commands

```sh
# Mock tab orchestration budgets (no network, no window):
cargo test -p halley-core --test tab_perf -- --nocapture

# Network/privacy foundation suites (no sockets):
cargo test -p halley-network -p halley-privacy --all-features

# Full suite / gates:
cargo test --workspace --all-features
```

There are **no `#[bench]` targets or criterion suites** in the repository
today (no extra bench dependency — AGENTS.md §8). `benches/` holds
documentation only.

## Manual native-tab memory procedure (**PLANNED** — not automated)

To measure real WebView2 per-tab cost (do not record a number without
this context):

1. Build release: `cargo build -p halley-core --release`
2. Start `halley` on `about:blank`; record machine/OS/toolchain
3. Open N tabs via Ctrl+T (1, 5, 10, 20); sample process private bytes
   (Task Manager / `Process Explorer`) after each step
4. Record: machine, OS, WebView2 runtime version, N, bytes, samples
5. Report as **baseline** only with the above fields attached

## What exists that could be measured manually

The `halley` binary can be timed by hand on a developer machine, for
example:

```sh
# illustrative only — not an automated gate
cargo build -p halley-core --release
# then start ./target/release/halley and observe startup visually
```
