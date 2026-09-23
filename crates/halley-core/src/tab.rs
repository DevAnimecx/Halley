//! Tab identity and per-tab display state.
//!
//! A `Tab` is core's user-facing unit; its content lives in an engine
//! page ([`halley_engine::PageId`]). Core never indexes tabs by strip
//! position for identity — positions change on reorder, ids do not.

use halley_engine::PageId;

/// Stable identity for a tab within a browser session.
///
/// Monotonically assigned by [`crate::tab_manager::TabManager`]; never
/// reused within a session. Serializes as the raw `u64` for snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TabId(u64);

impl TabId {
    /// Construct from a raw value (session restore / tests).
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Raw numeric value (IPC / snapshots).
    pub fn raw(self) -> u64 {
        self.0
    }

    /// Decimal string used in chrome IPC payloads.
    pub fn to_ipc_string(self) -> String {
        self.0.to_string()
    }

    /// Parse a decimal string produced by chrome IPC.
    ///
    /// Returns `None` for empty, non-numeric, or non-canonical input
    /// (leading `+`, spaces, etc. are rejected).
    pub fn from_ipc_str(s: &str) -> Option<Self> {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        // Reject leading zeros (except "0" itself) to keep ids canonical.
        if s.len() > 1 && s.starts_with('0') {
            return None;
        }
        s.parse::<u64>().ok().map(Self)
    }
}

/// Display / lifecycle state for one tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    id: TabId,
    page: PageId,
    url: String,
    title: String,
    loading: bool,
}

impl Tab {
    /// Create a tab bound to an engine page already loaded with `url`.
    pub fn new(id: TabId, page: PageId, url: impl Into<String>) -> Self {
        Self {
            id,
            page,
            url: url.into(),
            title: String::new(),
            loading: false,
        }
    }

    /// Tab identity.
    pub fn id(&self) -> TabId {
        self.id
    }

    /// Engine page backing this tab's content.
    pub fn page(&self) -> PageId {
        self.page
    }

    /// Last known content URL (address-bar value for this tab).
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Document title (may be empty).
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Whether a load is in progress for this tab.
    pub fn loading(&self) -> bool {
        self.loading
    }

    /// Display title: page title, else URL host/path, else a placeholder.
    pub fn display_title(&self) -> &str {
        if !self.title.is_empty() {
            return &self.title;
        }
        if self.url.is_empty() || self.url == "about:blank" {
            return "New Tab";
        }
        &self.url
    }

    /// Apply a URL change from the engine.
    pub fn set_url(&mut self, url: impl Into<String>) {
        self.url = url.into();
    }

    /// Apply a title change from the engine.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Apply a loading flag change from the engine.
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }
}

/// Maximum characters of a tab title pushed into chrome (beyond this the
/// strip relies on CSS ellipsis; keeps IPC payloads bounded).
pub const MAX_TAB_TITLE_CHARS: usize = 80;

/// Truncate a title for chrome IPC without splitting a char boundary.
pub fn truncate_title(title: &str) -> String {
    if title.chars().count() <= MAX_TAB_TITLE_CHARS {
        return title.to_string();
    }
    let cut: String = title.chars().take(MAX_TAB_TITLE_CHARS).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_id_ipc_round_trip() {
        let id = TabId::from_raw(42);
        assert_eq!(TabId::from_ipc_str(&id.to_ipc_string()), Some(id));
        assert_eq!(id.to_ipc_string(), "42");
    }

    #[test]
    fn tab_id_rejects_malformed_ipc() {
        assert_eq!(TabId::from_ipc_str(""), None);
        assert_eq!(TabId::from_ipc_str("abc"), None);
        assert_eq!(TabId::from_ipc_str("1 2"), None);
        assert_eq!(TabId::from_ipc_str("-1"), None);
        assert_eq!(TabId::from_ipc_str("+3"), None);
        assert_eq!(TabId::from_ipc_str("01"), None); // non-canonical
        assert_eq!(TabId::from_ipc_str("0"), Some(TabId::from_raw(0)));
    }

    #[test]
    fn display_title_falls_back_from_title_to_url_to_placeholder() {
        let mut tab = Tab::new(TabId::from_raw(1), 1, "about:blank");
        assert_eq!(tab.display_title(), "New Tab");
        tab.set_url("https://example.com/path");
        assert_eq!(tab.display_title(), "https://example.com/path");
        tab.set_title("Example");
        assert_eq!(tab.display_title(), "Example");
    }

    #[test]
    fn truncate_title_caps_length_with_ellipsis() {
        let long = "x".repeat(MAX_TAB_TITLE_CHARS + 10);
        let out = truncate_title(&long);
        assert_eq!(out.chars().count(), MAX_TAB_TITLE_CHARS + 1);
        assert!(out.ends_with('…'));

        assert_eq!(truncate_title("short"), "short");
    }

    #[test]
    fn truncate_title_is_char_boundary_safe() {
        let long: String = "日".repeat(MAX_TAB_TITLE_CHARS + 5);
        let out = truncate_title(&long);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= MAX_TAB_TITLE_CHARS + 1);
    }
}
