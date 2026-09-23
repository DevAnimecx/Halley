# Dev launch - build and open the Halley browser on this PC.
#
# Run from the repository root:
#   .\scripts\dev.ps1
#   .\scripts\dev.ps1 https://example.com
#   .\scripts\dev.ps1 -Release
#
# Mirrors README "Development setup" (cargo run -p halley-core --bin halley).
$ErrorActionPreference = "Stop"

$release = $false
$url = $null
foreach ($arg in $args) {
    if ($arg -eq "-Release" -or $arg -eq "--release") {
        $release = $true
    } elseif (-not $url) {
        $url = $arg
    } else {
        Write-Error "unexpected argument: $arg"
        exit 2
    }
}

$toolchain = Join-Path $PSScriptRoot "..\rust-toolchain.toml"
if (-not (Test-Path -LiteralPath $toolchain)) {
    Write-Error "rust-toolchain.toml not found - run from a Halley checkout"
    exit 1
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $cargoArgs = @("run", "-p", "halley-core", "--bin", "halley")
    if ($release) { $cargoArgs += "--release" }
    if ($url) { $cargoArgs += $url }

    $joined = $cargoArgs -join " "
    Write-Host "halley: cargo $joined" -ForegroundColor Cyan
    & cargo @cargoArgs
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
