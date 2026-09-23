# Scripts

Development helper scripts. Keep scripts small, deterministic, and free of
network side effects beyond `cargo` fetching declared dependencies.

| Script | Purpose |
| --- | --- |
| `verify.ps1` | Run the full local verification gate (fmt, check, clippy, test) on Windows |
| `verify.sh` | Same, for POSIX shells |

These mirror `.github/workflows/ci.yml`. If you change one, change the other.
