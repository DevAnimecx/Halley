//! Shared primitives for the Halley workspace.
//!
//! `halley-common` is the bottom of the dependency graph: every other crate
//! may depend on it, and it depends on no other Halley crate.
//!
//! It currently provides these foundations:
//!
//! * [`error`] — a minimal shared error strategy that lets subsystems report
//!   meaningful failures without coupling to each other's error types.
//! * [`config`] — configuration data types covering browser, privacy,
//!   network, cookies, storage, AI provider, Jerry, and performance
//!   settings. No secret storage exists yet; API keys are intentionally
//!   absent from these types.
//! * [`logging`] — a development logging initializer. Logs are local-only
//!   (stderr), never sent anywhere, and must never contain secrets.
//! * [`origin`] — strongly typed `scheme://host:port` origins shared by
//!   `halley-privacy` and `halley-network` (no sideways deps between them).

pub mod config;
pub mod error;
pub mod logging;
pub mod origin;

pub use config::{
    AppConfig, BrowserConfig, Config, CookieConfig, DnsConfig, NetworkConfig, ProxyConfig,
    SearchProviderConfig, StorageConfig, TlsConfig, WindowConfig,
};
pub use error::{Error, Result};
pub use origin::{same_site, site_label, Origin, OriginError};

/// Identifier for this subsystem, used by diagnostics and logging.
pub const SUBSYSTEM: &str = "halley-common";
