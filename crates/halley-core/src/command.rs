//! Typed browser commands and their outcomes.
//!
//! Chrome IPC [`halley_engine::ChromeAction`]s are mapped into
//! [`BrowserCommand`]s after tab-id parsing; tests and future MCP tools
//! can dispatch the same commands without going through IPC.

use crate::tab::TabId;

/// High-level commands the browser controller accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BrowserCommand {
    /// Open a new tab. `None` URL uses `config.browser.new_tab_url`.
    NewTab { url: Option<String> },
    /// Close the given tab.
    CloseTab(TabId),
    /// Focus/activate the given tab.
    ActivateTab(TabId),
    /// Next tab (wraps).
    NextTab,
    /// Previous tab (wraps).
    PrevTab,
    /// Reopen the most recently closed tab (no-op if stack empty).
    ReopenClosedTab,
    /// Open a new tab with the same URL as the source.
    DuplicateTab(TabId),
    /// Close every tab except the given one.
    CloseOtherTabs(TabId),
    /// Close tabs strictly to the right of the given tab.
    CloseTabsToRight(TabId),
    /// Reorder within the strip (destination clamped).
    MoveTab { tab_id: TabId, to_index: usize },
    /// Normalize address-bar input and navigate the active tab.
    NavigateInput(String),
    /// Navigate the active tab to an absolute URL.
    NavigateUrl(String),
    /// History back on the active tab.
    Back,
    /// History forward on the active tab.
    Forward,
    /// Reload the active tab.
    Reload,
    /// Request stop of an in-progress load.
    ///
    /// Always returns [`CommandOutcome::NotSupported`] on the current
    /// engine (wry has no `stop()`); the chrome UI does not offer Stop.
    Stop,
    /// Ask the chrome UI to focus/select the address bar.
    FocusAddressBar,
}

/// Result of a successful command dispatch (errors are [`crate::CoreError`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutcome {
    /// Command applied; state may have changed.
    Done,
    /// The command is recognized but not available on this engine/UI.
    NotSupported {
        /// Short operation name (e.g. `"stop"`).
        operation: &'static str,
        /// Human-readable reason (never contains secrets).
        reason: String,
    },
    /// Command was a no-op (e.g. reopen with empty stack).
    Noop,
}

impl CommandOutcome {
    /// Whether the command changed browser state.
    pub fn did_change_state(&self) -> bool {
        matches!(self, CommandOutcome::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcomes_expose_change_flag() {
        assert!(CommandOutcome::Done.did_change_state());
        assert!(!CommandOutcome::Noop.did_change_state());
        assert!(!CommandOutcome::NotSupported {
            operation: "stop",
            reason: "unsupported".into()
        }
        .did_change_state());
    }

    #[test]
    fn commands_carry_tab_ids_for_tab_ops() {
        let id = TabId::from_raw(7);
        assert_eq!(BrowserCommand::CloseTab(id), BrowserCommand::CloseTab(id));
        assert_eq!(
            BrowserCommand::MoveTab {
                tab_id: id,
                to_index: 2
            },
            BrowserCommand::MoveTab {
                tab_id: id,
                to_index: 2
            }
        );
    }
}
