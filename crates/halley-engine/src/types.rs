//! Engine-agnostic message and state types.
//!
//! These types are the only vocabulary that crosses the engine boundary
//! into `halley-core`. They contain no platform handles and no secrets.

/// Stable identifier for an engine page (one content web view).
///
/// Assigned by the engine on [`crate::BrowserEngine::create_page`].
/// Not a tab index; core maps `PageId` ↔ `TabId`.
pub type PageId = u64;

/// A user (or chrome UI) command originating from the embedded chrome.
///
/// Chrome IPC is the only producer; content pages have no IPC handler.
/// Core translates these into [`crate::BrowserCommand`]s (or their own
/// controller path) after parsing tab identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChromeAction {
    /// Load raw address-bar input (normalized by core).
    Navigate(String),
    /// History: one step back on the active page.
    Back,
    /// History: one step forward on the active page.
    Forward,
    /// Reload the active page.
    Reload,
    /// Request stop of an in-progress load (may be unsupported by engine).
    Stop,
    /// Open a new tab (URL comes from core config).
    NewTab,
    /// Close the tab whose id string was sent from chrome.
    CloseTab(String),
    /// Activate the tab whose id string was sent from chrome.
    ActivateTab(String),
    /// Activate the next tab (wraps).
    NextTab,
    /// Activate the previous tab (wraps).
    PrevTab,
    /// Reopen the most recently closed tab.
    ReopenClosedTab,
    /// Duplicate the given tab (fresh page, same URL).
    DuplicateTab(String),
    /// Focus and select the address bar content.
    FocusAddressBar,
    /// Close every tab except the referenced one.
    CloseOtherTabs(String),
    /// Close tabs to the right of the referenced tab (current order).
    CloseTabsToRight(String),
    /// Reorder: move tab to a destination index in the strip.
    MoveTab { tab_id: String, to_index: usize },
    /// Open or close the Jerry chat panel.
    JerryToggle,
    /// Send a chat message (freeform text after the verb).
    JerrySend(String),
    /// Cancel an in-flight provider stream.
    JerryStop,
    /// Start a new conversation thread.
    JerryNewConversation,
    /// Clear all conversations (local memory).
    JerryClear,
    /// Enable Jerry for this session.
    JerryEnable,
    /// Disable Jerry for this session (stops sends).
    JerryDisable,
    /// Set provider id (`openai`, `groq`, …).
    JerrySetProvider(String),
    /// Set model id within the provider catalog.
    JerrySetModel(String),
    /// Store the API key for the selected provider (never logged).
    JerrySetKey(String),
    /// Remove the stored API key for the selected provider.
    JerryClearKey,
    /// Run a minimal connectivity check against the provider.
    JerryTestProvider,
}

/// One chat message rendered in the Jerry panel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JerryChromeMessage {
    /// `"user"`, `"assistant"`, or `"system"`.
    pub role: String,
    /// Message text (already redacted/framed by Jerry before UI).
    pub content: String,
    /// Whether this is the in-flight assistant message still streaming.
    pub streaming: bool,
}

/// Jerry panel state pushed with chrome updates.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JerryChromeState {
    /// Whether the side panel is open (affects chrome height in core).
    pub open: bool,
    /// Whether Jerry is enabled for this session.
    pub enabled: bool,
    /// Provider id shown in the UI (empty if none).
    pub provider: String,
    /// Model id shown in the UI (empty if none).
    pub model: String,
    /// Whether a non-empty API key is stored (never the key itself).
    pub has_api_key: bool,
    /// `"idle" | "connecting" | "streaming" | "error" | "disabled"`.
    pub status: String,
    /// Conversation messages to render.
    pub messages: Vec<JerryChromeMessage>,
    /// Short context indicator (e.g. `"3 sources · ~120 tok"`).
    pub context_note: String,
    /// Last error message (sanitized; never secrets).
    pub error: Option<String>,
}

/// Page lifecycle and identity events emitted by the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    /// The page began loading `url`.
    PageStarted {
        /// Engine page identity.
        page: PageId,
        /// URL the engine reported for this load.
        url: String,
    },
    /// The page finished loading `url`.
    PageFinished {
        /// Engine page identity.
        page: PageId,
        /// URL the engine reported for this load.
        url: String,
    },
    /// The document title changed.
    TitleChanged {
        /// Engine page identity.
        page: PageId,
        /// New title (may be empty).
        title: String,
    },
    /// A content page requested a new window/tab (target=_blank, window.open).
    ///
    /// The engine **denies** native OS window creation; core decides to
    /// open a Halley tab instead (see ADR-007).
    NewWindowRequested {
        /// Page that made the request.
        page: PageId,
        /// Absolute URL the page asked to open (scheme-filtered by engine).
        url: String,
    },
}

/// Anything the engine hands back to core when messages are drained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineMessage {
    /// A page/title lifecycle event.
    Event(EngineEvent),
    /// A command originating from the embedded browser chrome (IPC).
    Action(ChromeAction),
}

/// Tab-strip entry for chrome rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromeTabState {
    /// Tab id as rendered/echoed by chrome (decimal string of `TabId`).
    pub id: String,
    /// Display title (truncated by core before push if needed).
    pub title: String,
    /// Whether this is the active tab.
    pub active: bool,
    /// Per-tab loading flag.
    pub loading: bool,
}

/// State the browser chrome should display.
///
/// Core owns this struct; the engine only serializes it into the chrome
/// web view.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChromeState {
    /// Address-bar value for the active tab.
    pub url: String,
    /// Document title for the active tab (also used for window title).
    pub title: String,
    /// Whether the back control should be enabled.
    pub can_go_back: bool,
    /// Whether the forward control should be enabled.
    pub can_go_forward: bool,
    /// Whether the active page is loading (toolbar shows Stop).
    pub loading: bool,
    /// Tab strip entries in display order.
    pub tabs: Vec<ChromeTabState>,
    /// Id of the active tab (empty if none — should not happen in UI).
    pub active_tab_id: String,
    /// Jerry chat panel state.
    pub jerry: JerryChromeState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_state_defaults_are_inert() {
        let state = ChromeState::default();
        assert!(state.url.is_empty());
        assert!(state.title.is_empty());
        assert!(!state.can_go_back);
        assert!(!state.can_go_forward);
        assert!(!state.loading);
        assert!(state.tabs.is_empty());
        assert!(state.active_tab_id.is_empty());
    }

    #[test]
    fn messages_round_trip_equality() {
        let started = EngineMessage::Event(EngineEvent::PageStarted {
            page: 1,
            url: "https://example.com/".into(),
        });
        assert_eq!(
            started,
            EngineMessage::Event(EngineEvent::PageStarted {
                page: 1,
                url: "https://example.com/".into()
            })
        );

        let action = EngineMessage::Action(ChromeAction::Navigate("https://example.com/".into()));
        assert!(matches!(
            action,
            EngineMessage::Action(ChromeAction::Navigate(_))
        ));
    }
}
