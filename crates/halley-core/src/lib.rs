//! Application orchestration and browser-level state for Halley.
//!
//! `halley-core` sits at the top of the dependency graph. It owns the
//! native window and event loop, drives the engine through the
//! [`halley_engine::BrowserEngine`] trait, normalizes address-bar input,
//! and keeps the multi-tab [`Browser`] session the chrome renders.
//!
//! Profile isolation / privacy policy come from `halley-privacy`;
//! Halley-originated HTTP goes through `halley-network` (no ad-hoc
//! sockets in core). Page loads still bypass those crates via the
//! platform WebView (documented gap until engine intercept).

pub mod application;
pub mod browser;
pub mod command;
pub mod error;
pub mod event;
pub mod jerry_host;
pub mod navigation;
pub mod session;
pub mod tab;
pub mod tab_manager;

pub use application::Application;
pub use browser::Browser;
pub use command::{BrowserCommand, CommandOutcome};
pub use error::CoreError;
pub use event::BrowserEvent;
pub use jerry_host::{apply_jerry_events, JerryHost, CONVERSATIONS_FILE_NAME};
pub use navigation::{normalize_address, NavigationError};
pub use session::{
    BrowserSession, SessionError, SessionId, SessionLifecycle, SessionSnapshot, TabSnapshot,
    SESSION_SCHEMA_VERSION,
};
pub use tab::{Tab, TabId};
pub use tab_manager::{ClosedTab, TabManager};

/// Identifier for this subsystem, used by diagnostics and logging.
pub const SUBSYSTEM: &str = "halley-core";

/// Chrome markup compiled into the binary (re-exported so the binary and
/// tests can load it without depending on wry types directly).
pub const CHROME_HTML: &str = include_str!("../../../assets/chrome.html");
