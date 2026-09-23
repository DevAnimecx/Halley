//! Privacy policies and protections.
//!
//! `halley-privacy` owns the **centralized privacy policy** for Halley:
//! cookies, storage, cache, referrer, permissions, site context, profiles,
//! and private-browsing isolation. Network *request decisions* live in
//! `halley-network` and are composed with these policies by `halley-core`
//! (ADR-002: no sideways dependency).
//!
//! Status (Prompt #4): policy types, cookie store, storage manager,
//! profile isolation, and private-session cleanup are **IMPLEMENTED** and
//! tested. Engine/WebView cookie jars and page storage are still owned by
//! the platform webview — this crate does not yet intercept them
//! (adapter wiring is future work). Tracker blocking, fingerprint
//! randomization, and DoH are **NOT IMPLEMENTED**.

pub mod cache;
pub mod cookie;
pub mod error;
pub mod party;
pub mod permission;
pub mod policy;
pub mod profile;
pub mod referrer;
pub mod site;
pub mod storage;

pub use cookie::{Cookie, CookiePolicy, CookieStore, SameSite};
pub use error::{CookieError, PrivacyError, ProfileError, StorageError};
pub use party::{classify_party, Party};
pub use permission::{Permission, PermissionDecision, PermissionPolicy, PermissionRequest};
pub use policy::PrivacyPolicy;
pub use profile::{BrowserProfile, ProfileId, ProfileKind};
pub use referrer::{referrer_value, ReferrerPolicy};
pub use site::SiteContext;
pub use storage::{safe_join_component, sanitize_component, StorageManager, StoragePaths};

/// Identifier for this subsystem, used by diagnostics and logging.
pub const SUBSYSTEM: &str = "halley-privacy";
