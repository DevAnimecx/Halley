# Verify — local CI gate.
#
# Mirrors .github/workflows/ci.yml. Run from the repository root.
$ErrorActionPreference = "Stop"
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo check --workspace --all-targets --all-features
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo clippy --workspace --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test --workspace --all-features
exit $LASTEXITCODE
