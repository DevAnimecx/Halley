# Tests

Workspace-level test layout.

- **Unit tests** live inside each crate next to the code they test
  (`#[cfg(test)] mod tests`).
- **Integration tests** live in each crate's `tests/` directory
  (e.g. `crates/halley-core/tests/workspace_smoke.rs`,
  `crates/halley-core/tests/browser_mock.rs`,
  `crates/halley-core/tests/tabs.rs`,
  `crates/halley-core/tests/commands.rs`,
  `crates/halley-core/tests/session.rs`,
  `crates/halley-core/tests/property_tabs.rs`,
  `crates/halley-core/tests/tab_perf.rs`,
  `crates/halley-core/tests/prompt4_regression.rs`,
  `crates/halley-core/tests/e2e_smoke.rs`,
  `crates/halley-network/tests/pipeline.rs`,
  `crates/halley-privacy/tests/private_isolation.rs`,
  `crates/halley-privacy/tests/property_isolation.rs`).
- Network tests use `MockTransport` / assert `NoTransport` — still **no
  sockets** in the default suite. Privacy tests use system temp dirs
  under unique names and clean up; they do not touch the user's real
  profile root when `storage.profile_root` is overridden.
- There is intentionally no root-level `tests/` crate: the repository root is
  a virtual Cargo workspace, and root-level `tests/` directories are ignored
  by Cargo. This directory is reserved for cross-crate test documentation and
  shared fixtures once later milestones need them.

Run everything:

```sh
cargo test --workspace --all-features
```

Ignored GUI end-to-end tests (require a display; spawn the real `halley`
binary against `about:blank`):

```sh
cargo test -p halley-core --test e2e_smoke -- --ignored
```

Rules:

- Tests must assert real behaviour of real code. Never hardcode expected
  values that merely mirror implementation details to force a pass.
- Never write tests that pretend unimplemented browser, network, or Jerry
  functionality exists.
- No test may perform real network I/O in the default suite. The mock
  engine (`halley-engine` `test-engine` feature) exists so orchestration
  tests need no window and no sockets; navigation tests only assert URL
  parsing (no fetches). Network pipeline tests inject `MockTransport`
  (scripted in-memory responses) or expect `NetworkError::NoTransport`.
