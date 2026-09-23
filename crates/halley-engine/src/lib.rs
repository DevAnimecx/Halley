//! Browser engine abstraction boundary.
//!
//! `halley-engine` is the seam between Halley and the concrete rendering
//! engine (ADR-006: **wry** hosting the platform WebView — WebView2 on
//! Windows).
//!
//! Responsibilities:
//!
//! * Multi-page engine trait ([`BrowserEngine`]): create/destroy/show
//!   content pages, navigate, history, layout, chrome state push.
//! * Engine-agnostic types for chrome IPC and page events.
//! * Containment of engine-facing failures behind [`EngineError`].
//!
//! What this crate does **not** do: tab policy, URL normalization,
//! window/event-loop ownership, session model, or network I/O of its own.

pub mod chrome_ipc;
pub mod error;
pub mod types;

#[cfg(feature = "native")]
mod wry_backend;

#[cfg(feature = "test-engine")]
pub mod mock;

pub use chrome_ipc::{chrome_state_script, parse_chrome_message};
pub use error::EngineError;
pub use types::{
    ChromeAction, ChromeState, ChromeTabState, EngineEvent, EngineMessage, JerryChromeMessage,
    JerryChromeState, PageId,
};

#[cfg(feature = "native")]
pub use wry_backend::WryBackend;

#[cfg(feature = "test-engine")]
pub use mock::MockEngine;

/// Identifier for this subsystem, used by diagnostics and logging.
pub const SUBSYSTEM: &str = "halley-engine";

/// Multi-page content engine: one chrome strip + N content pages.
///
/// Only the **active** page should be visible in the content area;
/// inactive pages stay alive (preserving form/JS state) when the backend
/// supports it.
pub trait BrowserEngine {
    /// Create a content page loading `url` (must already be absolute).
    fn create_page(&mut self, url: &str) -> Result<PageId, EngineError>;

    /// Destroy a page and release its engine resources.
    fn destroy_page(&mut self, page: PageId) -> Result<(), EngineError>;

    /// Show or hide a page's content view without destroying it.
    /// Exactly one page should be visible in the content area at a time.
    fn set_page_visible(&mut self, page: PageId, visible: bool) -> Result<(), EngineError>;

    /// Load `url` on `page`.
    fn navigate(&mut self, page: PageId, url: &str) -> Result<(), EngineError>;

    /// Step back one entry in `page` session history.
    fn back(&mut self, page: PageId) -> Result<(), EngineError>;

    /// Step forward one entry in `page` session history.
    fn forward(&mut self, page: PageId) -> Result<(), EngineError>;

    /// Reload `page`.
    fn reload(&mut self, page: PageId) -> Result<(), EngineError>;

    /// Request stop of an in-progress load on `page`.
    ///
    /// Backends without a stop API return [`EngineError::Unsupported`].
    fn stop(&mut self, page: PageId) -> Result<(), EngineError>;

    /// Whether back navigation is possible on `page`.
    fn can_go_back(&self, page: PageId) -> bool;

    /// Whether forward navigation is possible on `page`.
    fn can_go_forward(&self, page: PageId) -> bool;

    /// Last known URL for `page` (empty string if unknown).
    fn page_url(&self, page: PageId) -> &str;

    /// Last known document title for `page`.
    fn page_title(&self, page: PageId) -> &str;

    /// Whether a load is in progress on `page` (best-effort).
    fn page_is_loading(&self, page: PageId) -> bool;

    /// Push chrome UI state into the embedded chrome view.
    fn update_chrome(&mut self, state: &ChromeState) -> Result<(), EngineError>;

    /// Re-layout chrome and the content area for a window of
    /// `window_width` × `window_height` logical pixels with chrome of
    /// `chrome_height` logical pixels at the top.
    fn set_layout(
        &mut self,
        window_width: f64,
        window_height: f64,
        chrome_height: f64,
    ) -> Result<(), EngineError>;

    /// Take all pending engine messages (events and chrome actions).
    fn drain_messages(&mut self) -> Vec<EngineMessage>;

    /// Move keyboard focus into the currently visible content page.
    fn focus_active_page(&mut self) -> Result<(), EngineError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsystem_identity_is_stable() {
        assert_eq!(SUBSYSTEM, "halley-engine");
    }
}
