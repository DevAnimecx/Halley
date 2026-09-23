//! Network infrastructure: request model, policy, manager, typed errors.
//!
//! `halley-network` is the choke-point crate for Halley-originated HTTP
//! (ADR-008). The platform WebView still performs page loads itself — that
//! bypass remains a documented gap until a future engine-intercept
//! milestone. Nothing in this crate opens sockets by default: the default
//! [`Transport`] is [`NoTransport`], and tests use [`MockTransport`] or
//! policy-only paths.
//!
//! Responsibilities **implemented** here:
//! * Typed [`NetworkRequest`] / [`RequestContext`] / [`NetworkResponse`]
//! * [`NetworkPolicy`] allow/block/modify decisions (adblock plugs in later)
//! * Timeouts, redirect limits, user-agent, DNS/proxy/TLS **configuration**
//! * Privacy-safe logging via [`redact`]
//! * Typed [`NetworkError`]
//!
//! Not implemented: DoH, real HTTP client, proxy dialing, engine traffic
//! interception, tracker blocking (that is `halley-adblock`).

pub mod context;
pub mod dns;
pub mod error;
pub mod manager;
pub mod policy;
pub mod proxy;
pub mod redact;
pub mod redirect;
pub mod request;
pub mod timeout;
pub mod tls;
pub mod transport;
pub mod user_agent;

pub use context::{RequestContext, ResourceType};
pub use dns::DnsPolicy;
pub use error::NetworkError;
pub use manager::{NetworkManager, NetworkResponse};
pub use policy::{NetworkPolicy, RequestDecision};
pub use proxy::ProxySettings;
pub use redact::{redact_headers_for_log, redact_url_for_log, REDACTED};
pub use redirect::{RedirectDecision, RedirectTracker};
pub use request::{HeaderName, HttpMethod, NetworkRequest};
pub use timeout::TimeoutSettings;
pub use tls::TlsPolicy;
pub use transport::{MockTransport, NoTransport, Transport, TransportRequest, TransportResponse};
pub use user_agent::UserAgentPolicy;

/// Identifier for this subsystem, used by diagnostics and logging.
pub const SUBSYSTEM: &str = "halley-network";
