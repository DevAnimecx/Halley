//! Typed command dispatch tests (shared chrome / tests / future MCP path).

use halley_common::Config;
use halley_core::{Browser, BrowserCommand, CommandOutcome, SessionLifecycle, TabId};
use halley_engine::MockEngine;

fn browser() -> Browser<MockEngine> {
    Browser::new(MockEngine::new(), Config::default(), "about:blank").expect("initial tab")
}

#[test]
fn navigate_input_command_normalizes_and_completes() {
    let mut browser = browser();
    let out = browser
        .execute(BrowserCommand::NavigateInput("example.com".into()))
        .unwrap();
    assert_eq!(out, CommandOutcome::Done);
    assert_eq!(browser.active_url(), "https://example.com/");
}

#[test]
fn back_without_history_is_noop_not_error() {
    let mut browser = browser();
    let out = browser.execute(BrowserCommand::Back).unwrap();
    assert_eq!(out, CommandOutcome::Noop);
}

#[test]
fn commands_rejected_before_ready_and_after_closing() {
    let mut browser = browser();
    // Force non-ready lifecycle.
    {
        let session = browser.session_mut();
        session.set_lifecycle(SessionLifecycle::Created);
    }
    let err = browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap_err();
    assert!(matches!(err, halley_core::CoreError::Session(_)));

    browser.session_mut().set_lifecycle(SessionLifecycle::Ready);
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();

    browser.begin_close();
    let err = browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap_err();
    assert!(matches!(err, halley_core::CoreError::Session(_)));
}

#[test]
fn move_tab_unknown_id_is_noop() {
    let mut browser = browser();
    let out = browser
        .execute(BrowserCommand::MoveTab {
            tab_id: TabId::from_raw(9999),
            to_index: 0,
        })
        .unwrap();
    assert_eq!(out, CommandOutcome::Noop);
}

#[test]
fn focus_address_command_completes_without_engine_navigation() {
    let mut browser = browser();
    let before = browser.engine().commands.len();
    let out = browser.execute(BrowserCommand::FocusAddressBar).unwrap();
    assert_eq!(out, CommandOutcome::Done);
    assert_eq!(browser.engine().commands.len(), before);
}
