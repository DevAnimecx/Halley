//! Browser-level events emitted after state changes.
//!
//! The application loop uses these for window-title sync and logging;
//! future UI/MCP layers can observe the same stream without reading
//! engine internals.

use halley_engine::ChromeAction;

use crate::session::SessionLifecycle;
use crate::tab::TabId;

/// Events observable outside [`crate::Browser`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BrowserEvent {
    /// A tab was created and inserted into the strip.
    TabCreated(TabId),
    /// A tab was closed (and remembered for reopen, if configured).
    TabClosed(TabId),
    /// The active tab changed (id of the new active tab).
    TabActivated(TabId),
    /// Tabs were reordered (`from` strip index → `to` strip index).
    TabMoved { from: usize, to: usize },
    /// Session lifecycle changed.
    SessionLifecycleChanged {
        /// Previous state.
        from: SessionLifecycle,
        /// New state.
        to: SessionLifecycle,
    },
    /// Active (or observed) tab started loading `url`.
    PageLoadStarted { tab: TabId, url: String },
    /// Page load finished for `tab`.
    PageLoadFinished { tab: TabId, url: String },
    /// Document title changed for `tab`.
    TitleChanged { tab: TabId, title: String },
    /// Content asked for a new window; core opens a Halley tab (ADR-007).
    NewWindowRequested { tab: TabId, url: String },
    /// Chrome Jerry panel action for `JerryHost` (not a tab command).
    JerryAction(ChromeAction),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_carry_tab_identity() {
        let id = TabId::from_raw(3);
        assert_eq!(BrowserEvent::TabCreated(id), BrowserEvent::TabCreated(id));
        assert_eq!(
            BrowserEvent::PageLoadStarted {
                tab: id,
                url: "https://example.com/".into()
            },
            BrowserEvent::PageLoadStarted {
                tab: id,
                url: "https://example.com/".into()
            }
        );
    }
}
