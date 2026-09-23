//! WASM extension sandbox.
//!
//! `halley-extensions` will host third-party extensions as sandboxed WASM
//! modules with capability-based access to browser APIs. Extensions are
//! untrusted code: they must not reach the filesystem, arbitrary network,
//! or secrets without explicit grants (see `docs/security-model.md`).
//!
//! Genesis status: **not implemented**. No runtime, no extension loading.

/// Identifier for this subsystem, used by diagnostics and logging.
pub const SUBSYSTEM: &str = "halley-extensions";
