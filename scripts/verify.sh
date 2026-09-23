#!/usr/bin/env sh
# Verify — local CI gate. Mirrors .github/workflows/ci.yml. Run from repo root.
set -eu
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
