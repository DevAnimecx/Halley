//! Multi-tab lifecycle integration tests (MockEngine, no network).

use halley_common::Config;
use halley_core::{Browser, BrowserCommand, CommandOutcome, TabId};
use halley_engine::{ChromeAction, EngineEvent, MockEngine};

fn browser() -> Browser<MockEngine> {
    Browser::new(MockEngine::new(), Config::default(), "about:blank").expect("initial tab")
}

fn tab_count(browser: &Browser<MockEngine>) -> usize {
    browser.session().tabs().len()
}

#[test]
fn new_tab_opens_with_config_url_and_becomes_active() {
    let mut browser = browser();
    let first = browser.active_tab_id().unwrap();
    let id = browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    assert_eq!(id, CommandOutcome::Done);
    let second = browser.active_tab_id().unwrap();
    assert_ne!(first, second);
    assert_eq!(tab_count(&browser), 2);
    assert_eq!(browser.active_url(), "about:blank");
    // Both pages exist in the mock.
    let p1 = browser.session().tabs().get(first).unwrap().page();
    let p2 = browser.session().tabs().get(second).unwrap().page();
    assert!(browser.engine().has_page(p1));
    assert!(browser.engine().has_page(p2));
}

#[test]
fn close_tab_activates_neighbor_and_destroys_page() {
    let mut browser = browser();
    let a = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let b = browser.active_tab_id().unwrap();
    let page_b = browser.session().tabs().get(b).unwrap().page();

    let outcome = browser.execute(BrowserCommand::CloseTab(b)).unwrap();
    assert_eq!(outcome, CommandOutcome::Done);
    assert_eq!(tab_count(&browser), 1);
    assert_eq!(browser.active_tab_id(), Some(a));
    assert!(!browser.engine().has_page(page_b));
}

#[test]
fn closing_last_tab_reopens_fresh_tab_never_empty_strip() {
    let mut browser = browser();
    let only = browser.active_tab_id().unwrap();
    let old_page = browser.session().tabs().get(only).unwrap().page();
    browser.execute(BrowserCommand::CloseTab(only)).unwrap();

    assert_eq!(tab_count(&browser), 1, "strip must never be empty");
    assert_ne!(browser.active_tab_id(), Some(only));
    assert!(!browser.engine().has_page(old_page));
    assert_eq!(browser.active_url(), "about:blank");
}

#[test]
fn next_and_prev_tab_wrap() {
    let mut browser = browser();
    let a = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let b = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let c = browser.active_tab_id().unwrap();

    browser.execute(BrowserCommand::NextTab).unwrap();
    // From c, next wraps to a (only tab with that id still open).
    assert_eq!(browser.active_tab_id(), Some(a));
    browser.execute(BrowserCommand::PrevTab).unwrap();
    assert_eq!(browser.active_tab_id(), Some(c));
    let _ = b;
}

#[test]
fn chrome_activate_tab_action_targets_specific_id() {
    let mut browser = browser();
    let first = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let second = browser.active_tab_id().unwrap();
    assert_ne!(first, second);

    browser
        .engine_mut()
        .inject_chrome_action(ChromeAction::ActivateTab(first.to_ipc_string()));
    browser.process_messages().unwrap();
    assert_eq!(browser.active_tab_id(), Some(first));
}

#[test]
fn chrome_close_with_bad_id_is_ignored() {
    let mut browser = browser();
    browser
        .engine_mut()
        .inject_chrome_action(ChromeAction::CloseTab("not-a-number".into()));
    browser
        .engine_mut()
        .inject_chrome_action(ChromeAction::CloseTab("".into()));
    browser.process_messages().unwrap();
    assert_eq!(tab_count(&browser), 1);
}

#[test]
fn reopen_closed_tab_restores_url_as_new_tab() {
    let mut browser = browser();
    browser
        .execute(BrowserCommand::NavigateInput("https://example.com/".into()))
        .unwrap();
    browser.process_messages().unwrap();
    let doomed = browser.active_tab_id().unwrap();
    browser.execute(BrowserCommand::CloseTab(doomed)).unwrap();
    // Last-tab close already opened a fresh about:blank tab.
    assert_eq!(browser.active_url(), "about:blank");

    let reopened = browser.execute(BrowserCommand::ReopenClosedTab).unwrap();
    assert_eq!(reopened, CommandOutcome::Done);
    assert_eq!(tab_count(&browser), 2);
    // Closed URL was example.com (plus initial about:blank stack).
    // Most recently closed is the example.com tab.
    assert_eq!(browser.active_url(), "https://example.com/");
}

#[test]
fn reopen_with_empty_stack_is_noop() {
    let mut browser = browser();
    let before = tab_count(&browser);
    let outcome = browser.execute(BrowserCommand::ReopenClosedTab).unwrap();
    assert_eq!(outcome, CommandOutcome::Noop);
    assert_eq!(tab_count(&browser), before);
}

#[test]
fn duplicate_tab_copies_url_and_activates_copy() {
    let mut browser = browser();
    browser
        .execute(BrowserCommand::NavigateInput("https://example.com/".into()))
        .unwrap();
    browser.process_messages().unwrap();
    let source = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::DuplicateTab(source))
        .unwrap();
    assert_eq!(tab_count(&browser), 2);
    assert_eq!(browser.active_url(), "https://example.com/");
    assert_ne!(browser.active_tab_id(), Some(source));
}

#[test]
fn close_other_tabs_keeps_keep() {
    let mut browser = browser();
    let a = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let b = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let c = browser.active_tab_id().unwrap();

    browser.execute(BrowserCommand::CloseOtherTabs(c)).unwrap();
    assert_eq!(tab_count(&browser), 1);
    assert_eq!(browser.active_tab_id(), Some(c));
    let _ = a;
    let _ = b;
}

#[test]
fn close_to_right_closes_suffix() {
    let mut browser = browser();
    let a = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let _b = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let _c = browser.active_tab_id().unwrap();

    browser.execute(BrowserCommand::ActivateTab(a)).unwrap();
    browser
        .execute(BrowserCommand::CloseTabsToRight(a))
        .unwrap();
    assert_eq!(tab_count(&browser), 1);
    assert_eq!(browser.active_tab_id(), Some(a));
}

#[test]
fn move_tab_reorders_strip_preserving_ids() {
    let mut browser = browser();
    let a = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let b = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let c = browser.active_tab_id().unwrap();

    let order_before: Vec<TabId> = browser
        .session()
        .tabs()
        .tabs()
        .iter()
        .map(|t| t.id())
        .collect();
    assert_eq!(order_before, vec![a, b, c]);

    browser
        .execute(BrowserCommand::MoveTab {
            tab_id: c,
            to_index: 0,
        })
        .unwrap();
    let order: Vec<TabId> = browser
        .session()
        .tabs()
        .tabs()
        .iter()
        .map(|t| t.id())
        .collect();
    assert_eq!(order, vec![c, a, b]);
}

#[test]
fn background_events_route_to_the_right_tab() {
    let mut browser = browser();
    let first = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    let second = browser.active_tab_id().unwrap();
    let page_first = browser.session().tabs().get(first).unwrap().page();
    let page_second = browser.session().tabs().get(second).unwrap().page();

    browser
        .engine_mut()
        .inject_event(EngineEvent::TitleChanged {
            page: page_first,
            title: "First".into(),
        });
    browser
        .engine_mut()
        .inject_event(EngineEvent::TitleChanged {
            page: page_second,
            title: "Second".into(),
        });
    browser.process_messages().unwrap();

    assert_eq!(
        browser.session().tabs().get(first).unwrap().title(),
        "First"
    );
    assert_eq!(
        browser.session().tabs().get(second).unwrap().title(),
        "Second"
    );
}

#[test]
fn new_window_request_opens_halley_tab() {
    let mut browser = browser();
    let before = tab_count(&browser);
    let page = browser.active_tab().unwrap().page();
    browser
        .engine_mut()
        .inject_event(EngineEvent::NewWindowRequested {
            page,
            url: "https://popup.example/".into(),
        });
    browser.process_messages().unwrap();
    assert_eq!(tab_count(&browser), before + 1);
    assert_eq!(browser.active_url(), "https://popup.example/");
}

#[test]
fn chrome_state_lists_all_tabs_with_one_active() {
    let mut browser = browser();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    browser.push_chrome().unwrap();
    let chrome = browser.engine().last_chrome.clone().unwrap();
    assert_eq!(chrome.tabs.len(), 3);
    let actives: Vec<bool> = chrome.tabs.iter().map(|t| t.active).collect();
    assert_eq!(actives.iter().filter(|a| **a).count(), 1);
    assert_eq!(
        chrome.active_tab_id,
        chrome.tabs.iter().find(|t| t.active).unwrap().id
    );
}
