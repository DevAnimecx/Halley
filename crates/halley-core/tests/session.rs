//! Session lifecycle + snapshot serialization tests.

use halley_common::Config;
use halley_core::{
    Browser, BrowserSession, SessionError, SessionLifecycle, SessionSnapshot, Tab, TabSnapshot,
    SESSION_SCHEMA_VERSION,
};
use halley_engine::MockEngine;

#[test]
fn browser_starts_ready_session_with_generated_id() {
    let browser = Browser::new(MockEngine::new(), Config::default(), "about:blank").unwrap();
    assert_eq!(browser.session().lifecycle(), SessionLifecycle::Ready);
    assert!(browser.session().id().as_str().starts_with("session-"));
    assert!(browser.session().accepts_commands());
}

#[test]
fn snapshot_of_live_browser_round_trips_through_json() {
    let mut browser = Browser::new(MockEngine::new(), Config::default(), "about:blank").unwrap();
    browser
        .execute(halley_core::BrowserCommand::NewTab { url: None })
        .unwrap();
    browser
        .execute(halley_core::BrowserCommand::NavigateInput(
            "https://example.com/".into(),
        ))
        .unwrap();
    browser.process_messages().unwrap();

    let snap = browser.session().to_snapshot();
    assert_eq!(snap.schema_version, SESSION_SCHEMA_VERSION);
    assert_eq!(snap.tabs.len(), 2);

    let json = snap.to_json().unwrap();
    let parsed = SessionSnapshot::from_json(&json).unwrap();
    let restored = BrowserSession::from_parsed(parsed, browser.config().browser.max_closed_tabs)
        .expect("restore");
    assert_eq!(restored.id(), browser.session().id());
    assert_eq!(restored.lifecycle(), SessionLifecycle::Created);
}

#[test]
fn restore_rejects_future_schema() {
    let snap = SessionSnapshot {
        schema_version: SESSION_SCHEMA_VERSION + 1,
        session_id: "session-legacy".into(),
        tabs: vec![TabSnapshot {
            url: "about:blank".into(),
            title: String::new(),
        }],
        active_index: 0,
    };
    let err = BrowserSession::from_parsed(snap, 5).unwrap_err();
    assert!(matches!(err, SessionError::UnsupportedSchema { .. }));
}

#[test]
fn tab_manager_invariants_hold_after_many_ops() {
    use halley_core::TabManager;
    let mut m = TabManager::new(3);
    let mut ids = Vec::new();
    for i in 0..10 {
        let id = m.alloc_id();
        m.push_active(Tab::new(id, i + 1, format!("https://t{i}.example/")));
        ids.push(id);
        m.assert_invariants();
    }
    for id in ids.iter().take(7) {
        m.close(*id);
        m.assert_invariants();
    }
    assert!(m.closed_len() <= 3);
}
