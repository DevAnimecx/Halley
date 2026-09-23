//! Browser session: identity, lifecycle, and tab collection.
//!
//! A session is the unit Jerry (later) and session persistence address.
//! Lifecycle is explicit so future work can gate commands on readiness
//! without inventing ad-hoc flags.
//!
//! Persistence: [`BrowserSession::to_snapshot`] / [`from_parsed`] are
//! **in-memory JSON foundation only** — no disk I/O, no auto-restore
//! (those are later milestones).

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::tab::{Tab, TabId};
use crate::tab_manager::TabManager;

/// Schema version written into / accepted by session snapshots.
///
/// Bump on any breaking change to [`SessionSnapshot`]; older versions are
/// rejected rather than partially interpreted.
pub const SESSION_SCHEMA_VERSION: u32 = 1;

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Opaque session identifier (process-local, monotonic).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Allocate a new unique session id (monotonic counter — no UUID dep).
    pub fn generate() -> Self {
        let n = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
        SessionId(format!("session-{n}"))
    }

    /// The id string (snapshots, logs — never secrets).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Session lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifecycle {
    /// Allocated but tabs not yet created.
    Created,
    /// Initial pages are being created.
    Initializing,
    /// Accepting commands from the UI.
    Ready,
    /// Running (alias of Ready for future agent attach points).
    Running,
    /// Shutting down: tabs are being torn down.
    Closing,
    /// Fully torn down; no further commands.
    Closed,
}

/// In-memory browser session: identity + lifecycle + tabs.
#[derive(Debug)]
pub struct BrowserSession {
    id: SessionId,
    lifecycle: SessionLifecycle,
    tabs: TabManager,
}

impl BrowserSession {
    /// New empty session in [`SessionLifecycle::Created`].
    pub fn new(max_closed_tabs: usize) -> Self {
        Self {
            id: SessionId::generate(),
            lifecycle: SessionLifecycle::Created,
            tabs: TabManager::new(max_closed_tabs),
        }
    }

    /// Session id.
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    /// Current lifecycle state.
    pub fn lifecycle(&self) -> SessionLifecycle {
        self.lifecycle
    }

    /// Transition lifecycle (returns previous state for logging/tests).
    pub fn set_lifecycle(&mut self, next: SessionLifecycle) -> SessionLifecycle {
        let prev = self.lifecycle;
        self.lifecycle = next;
        prev
    }

    /// Whether the session accepts user commands.
    pub fn accepts_commands(&self) -> bool {
        matches!(
            self.lifecycle,
            SessionLifecycle::Ready | SessionLifecycle::Running
        )
    }

    /// Tab manager (read access).
    pub fn tabs(&self) -> &TabManager {
        &self.tabs
    }

    /// Tab manager (mut access for the browser controller).
    pub fn tabs_mut(&mut self) -> &mut TabManager {
        &mut self.tabs
    }

    /// Insert a tab (used by [`crate::Browser`] when creating pages).
    pub fn insert_tab(&mut self, tab: Tab) -> TabId {
        self.tabs.push_active(tab)
    }

    /// Serializable snapshot of open tabs (not the closed stack — closed
    /// URLs are intentionally not persisted yet).
    pub fn to_snapshot(&self) -> SessionSnapshot {
        let tabs: Vec<TabSnapshot> = self
            .tabs
            .tabs()
            .iter()
            .map(|t| TabSnapshot {
                url: t.url().to_string(),
                title: t.title().to_string(),
            })
            .collect();
        let active_index = self
            .tabs
            .active_id()
            .and_then(|id| self.tabs.index_of(id))
            .unwrap_or(0);
        SessionSnapshot {
            schema_version: SESSION_SCHEMA_VERSION,
            session_id: self.id.as_str().to_string(),
            tabs,
            active_index,
        }
    }

    /// Rebuild a session shell from a snapshot.
    ///
    /// Engine pages and tab rows are **not** populated here — the caller
    /// ([`crate::Browser`]) must open a page per restored URL and insert
    /// tabs. Tab ids are reassigned fresh; `active_index` is validated
    /// against the snapshot before this returns `Ok`.
    pub fn from_parsed(
        snapshot: SessionSnapshot,
        max_closed_tabs: usize,
    ) -> Result<Self, SessionError> {
        if snapshot.schema_version != SESSION_SCHEMA_VERSION {
            return Err(SessionError::UnsupportedSchema {
                found: snapshot.schema_version,
                expected: SESSION_SCHEMA_VERSION,
            });
        }
        if snapshot.tabs.is_empty() {
            return Err(SessionError::EmptyTabs);
        }
        if snapshot.active_index >= snapshot.tabs.len() {
            return Err(SessionError::ActiveIndexOutOfRange {
                index: snapshot.active_index,
                len: snapshot.tabs.len(),
            });
        }
        Ok(Self {
            id: SessionId(snapshot.session_id),
            lifecycle: SessionLifecycle::Created,
            tabs: TabManager::new(max_closed_tabs),
        })
    }
}

/// Why a session snapshot could not be restored.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionError {
    /// `schema_version` is not the version this build understands.
    UnsupportedSchema { found: u32, expected: u32 },
    /// Snapshot contains zero tabs (sessions always have ≥ 1 tab).
    EmptyTabs,
    /// `active_index` does not address a tab in the snapshot.
    ActiveIndexOutOfRange { index: usize, len: usize },
    /// JSON parse failure (message is parser output — no secrets).
    Deserialize(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::UnsupportedSchema { found, expected } => {
                write!(
                    f,
                    "unsupported session schema {found} (expected {expected})"
                )
            }
            SessionError::EmptyTabs => write!(f, "session snapshot has no tabs"),
            SessionError::ActiveIndexOutOfRange { index, len } => {
                write!(f, "active_index {index} out of range (len {len})")
            }
            SessionError::Deserialize(msg) => write!(f, "session deserialize: {msg}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// One open tab in a snapshot (no engine page id — pages are recreated).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabSnapshot {
    /// URL to reload when restoring.
    pub url: String,
    /// Last known title (display until load).
    pub title: String,
}

/// Portable session snapshot (JSON via serde).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    /// Format version — see [`SESSION_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Session id string (stable across save/load of the same session).
    pub session_id: String,
    /// Open tabs in strip order.
    pub tabs: Vec<TabSnapshot>,
    /// Index of the active tab in `tabs`.
    pub active_index: usize,
}

impl SessionSnapshot {
    /// Serialize to JSON (pretty for future human-readable saves).
    pub fn to_json(&self) -> Result<String, SessionError> {
        serde_json::to_string_pretty(self).map_err(|err| SessionError::Deserialize(err.to_string()))
    }

    /// Parse JSON produced by [`to_json`](Self::to_json).
    pub fn from_json(json: &str) -> Result<Self, SessionError> {
        serde_json::from_str(json).map_err(|err| SessionError::Deserialize(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_is_created_and_empty_until_tabs_inserted() {
        let s = BrowserSession::new(5);
        assert_eq!(s.lifecycle(), SessionLifecycle::Created);
        assert!(!s.accepts_commands());
        assert!(s.tabs().is_empty());
        assert!(s.id().as_str().starts_with("session-"));
    }

    #[test]
    fn lifecycle_transitions_track_previous_state() {
        let mut s = BrowserSession::new(5);
        assert_eq!(
            s.set_lifecycle(SessionLifecycle::Initializing),
            SessionLifecycle::Created
        );
        assert_eq!(
            s.set_lifecycle(SessionLifecycle::Ready),
            SessionLifecycle::Initializing
        );
        assert!(s.accepts_commands());
        s.set_lifecycle(SessionLifecycle::Closing);
        assert!(!s.accepts_commands());
        s.set_lifecycle(SessionLifecycle::Closed);
        assert!(!s.accepts_commands());
    }

    #[test]
    fn snapshot_round_trips_schema_version_and_tabs() {
        let mut s = BrowserSession::new(3);
        let id = s.tabs_mut().alloc_id();
        s.insert_tab(Tab::new(id, id.raw(), "https://a.example/"));
        let id2 = s.tabs_mut().alloc_id();
        s.insert_tab(Tab::new(id2, id2.raw(), "https://b.example/"));
        s.tabs_mut().activate(id); // active is first, not last

        let snap = s.to_snapshot();
        assert_eq!(snap.schema_version, SESSION_SCHEMA_VERSION);
        assert_eq!(snap.tabs.len(), 2);
        assert_eq!(snap.active_index, 0);
        assert_eq!(snap.tabs[0].url, "https://a.example/");

        let json = snap.to_json().unwrap();
        let restored = SessionSnapshot::from_json(&json).unwrap();
        assert_eq!(restored, snap);

        let session = BrowserSession::from_parsed(restored, 3).unwrap();
        assert_eq!(session.id(), s.id());
        assert_eq!(session.lifecycle(), SessionLifecycle::Created);
        assert!(session.tabs().is_empty()); // pages recreated by Browser
    }

    #[test]
    fn rejects_wrong_schema_version() {
        let snap = SessionSnapshot {
            schema_version: 999,
            session_id: "session-x".into(),
            tabs: vec![TabSnapshot {
                url: "about:blank".into(),
                title: String::new(),
            }],
            active_index: 0,
        };
        assert_eq!(
            BrowserSession::from_parsed(snap, 1).unwrap_err(),
            SessionError::UnsupportedSchema {
                found: 999,
                expected: SESSION_SCHEMA_VERSION
            }
        );
    }

    #[test]
    fn rejects_empty_tabs_and_bad_active_index() {
        let empty = SessionSnapshot {
            schema_version: SESSION_SCHEMA_VERSION,
            session_id: "s".into(),
            tabs: vec![],
            active_index: 0,
        };
        assert_eq!(
            BrowserSession::from_parsed(empty, 1).unwrap_err(),
            SessionError::EmptyTabs
        );

        let bad = SessionSnapshot {
            schema_version: SESSION_SCHEMA_VERSION,
            session_id: "s".into(),
            tabs: vec![TabSnapshot {
                url: "about:blank".into(),
                title: String::new(),
            }],
            active_index: 5,
        };
        assert_eq!(
            BrowserSession::from_parsed(bad, 1).unwrap_err(),
            SessionError::ActiveIndexOutOfRange { index: 5, len: 1 }
        );
    }

    #[test]
    fn rejects_corrupt_json() {
        assert!(matches!(
            SessionSnapshot::from_json("{not json"),
            Err(SessionError::Deserialize(_))
        ));
    }
}
