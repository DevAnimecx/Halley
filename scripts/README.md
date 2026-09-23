# Scripts

Development helper scripts. Keep scripts small, deterministic, and free of
network side effects beyond `cargo` fetching declared dependencies.

| Script | Purpose |
| --- | --- |
| `verify.ps1` | Run the full local verification gate (fmt, check, clippy, test) on Windows |
| `verify.sh` | Same, for POSIX shells |
| `dev.ps1` | Build and open the Halley browser on this PC (optional URL, `-Release`) |

These mirror `.github/workflows/ci.yml`. If you change one, change the other.

## Dev launch (Windows)

```powershell
.\scripts\dev.ps1                              # debug build, homepage
.\scripts\dev.ps1 https://example.com          # open a URL/search
.\scripts\dev.ps1 -Release                     # release build
```

Equivalent without the script:

```powershell
cargo run -p halley-core --bin halley
cargo run -p halley-core --bin halley https://example.com
```

