# Benchmarks

Performance benchmark home for Halley.

**Status: no criterion `benches/` targets exist yet.** Wall-clock
sanity tests for mock tab orchestration live in
`crates/halley-core/tests/tab_perf.rs` (generous budgets, not
regression baselines). No native WebView memory numbers are claimed. See
[`docs/performance-baseline.md`](../docs/performance-baseline.md).

## Future approach

When real subsystems exist (network pipeline, adblock matching, rendering),
benchmarks will be added with [`criterion`](https://github.com/bheisler/criterion.rs)
as dev-dependencies of the crate being measured, following Cargo's
`benches/` convention inside that crate (e.g.
`crates/halley-adblock/benches/filter_lookup.rs`).

Rules:

- Benchmarks measure real code paths only.
- Never publish or commit fabricated performance numbers.
- Never compare Halley to other browsers without a reproducible methodology
  documented alongside the numbers.
- CI does not gate on benchmark results at this stage; benchmarks are run
  manually and their environment recorded.
