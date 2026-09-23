//! Integration tests: Browser + MockEngine over the public API.
//!
//! These exercise the orchestration path core uses in production
//! (navigate → drain → state sync → chrome push) without opening a
//! window. Real GUI behavior is covered by the ignored e2e smoke test.

use halley_common::Config;
use halley_core::{normalize_address, Browser, BrowserCommand, CommandOutcome, CoreError};
use halley_engine::{ChromeAction, EngineEvent, MockEngine};

fn browser() -> Browser<MockEngine> {
    Browser::new(MockEngine::new(), Config::default(), "about:blank").expect("initial tab")
}

fn active_page(browser: &Browser<MockEngine>) -> u64 {
    browser.active_tab().expect("active tab").page()
}

#[test]
fn full_navigate_load_title_cycle() {
    let mut browser = browser();
    let page = active_page(&browser);
    browser.navigate_input("example.com").unwrap();
    assert!(browser.active_tab().unwrap().loading());

    let page_for_event = page;
    browser
        .engine_mut()
        .inject_event(EngineEvent::TitleChanged {
            page: page_for_event,
            title: "Example Domain".into(),
        });
    browser
        .engine_mut()
        .inject_event(EngineEvent::PageFinished {
            page: page_for_event,
            url: "https://example.com/".into(),
        });
    browser.process_messages().unwrap();

    let tab = browser.active_tab().unwrap();
    assert_eq!(tab.url(), "https://example.com/");
    assert_eq!(tab.title(), "Example Domain");
    assert!(!tab.loading());

    let chrome = browser.engine().last_chrome.clone().expect("chrome push");
    assert_eq!(chrome.url, "https://example.com/");
    assert_eq!(chrome.title, "Example Domain");
    assert!(!chrome.loading);
    assert_eq!(chrome.tabs.len(), 1);
    assert!(chrome.tabs[0].active);
}

#[test]
fn multi_step_history_walk_end_to_end() {
    let mut browser = browser();
    let page = active_page(&browser);
    for path in ["a.example", "b.example", "c.example"] {
        browser.navigate_input(path).unwrap();
        browser.process_messages().unwrap();
    }
    assert_eq!(
        browser.engine().history_urls(page),
        [
            "about:blank",
            "https://a.example/",
            "https://b.example/",
            "https://c.example/"
        ]
    );

    browser
        .engine_mut()
        .inject_chrome_action(ChromeAction::Back);
    browser.process_messages().unwrap();
    assert_eq!(browser.active_url(), "https://b.example/");
    assert!(browser.can_go_back());
    assert!(browser.can_go_forward());

    browser
        .engine_mut()
        .inject_chrome_action(ChromeAction::Back);
    browser.process_messages().unwrap();
    assert_eq!(browser.active_url(), "https://a.example/");
    assert!(browser.can_go_back());
    assert!(browser.can_go_forward());

    browser
        .engine_mut()
        .inject_chrome_action(ChromeAction::Forward);
    browser.process_messages().unwrap();
    assert_eq!(browser.active_url(), "https://b.example/");
    assert!(browser.can_go_back());
    assert!(browser.can_go_forward());

    let chrome = browser.engine().last_chrome.clone().expect("chrome push");
    assert_eq!(chrome.url, "https://b.example/");
    assert!(chrome.can_go_back);
    assert!(chrome.can_go_forward);
}

#[test]
fn chrome_back_after_single_entry_is_tolerated() {
    // Initial about:blank is the only history entry — back is a no-op.
    let mut browser = browser();
    browser
        .engine_mut()
        .inject_chrome_action(ChromeAction::Back);
    browser.process_messages().unwrap();
    assert_eq!(browser.active_url(), "about:blank");
    assert!(!browser.can_go_back());
}

#[test]
fn address_bar_search_input_reaches_engine_as_ddg_url() {
    let mut browser = browser();
    let page = active_page(&browser);
    browser.navigate_input("halley browser rust").unwrap();
    let urls = browser.engine().history_urls(page);
    // Initial about:blank + the search navigation.
    assert_eq!(urls.len(), 2);
    assert!(urls[1].starts_with("https://duckduckgo.com/?q="));
    assert!(urls[1].contains("halley%20browser%20rust") || urls[1].contains("halley+browser+rust"));
}

#[test]
fn javascript_scheme_never_reaches_the_engine() {
    let mut browser = browser();
    let commands_before = browser.engine().commands.len();
    let err = browser
        .navigate_input("javascript:alert(document.cookie)")
        .unwrap_err();
    assert!(matches!(err, CoreError::Navigation(_)));
    // Only the initial create_page command from Browser::new — no navigate.
    let page = active_page(&browser);
    assert_eq!(browser.engine().history_urls(page), ["about:blank"]);
    assert_eq!(browser.engine().commands.len(), commands_before);
}

#[test]
fn layout_calls_reach_engine_with_config_chrome_height() {
    let mut browser = browser();
    browser.set_layout(800.0, 600.0).unwrap();
    let (w, h, chrome) = browser.engine().last_layout.unwrap();
    assert_eq!((w, h), (800.0, 600.0));
    assert_eq!(chrome, f64::from(Config::default().window.chrome_height));
}

#[test]
fn normalize_address_and_browser_agree() {
    let search = &Config::default().browser.search_provider;
    assert_eq!(
        normalize_address("example.com", search).unwrap(),
        "https://example.com/"
    );
    let mut browser = browser();
    browser.navigate_input("example.com").unwrap();
    assert_eq!(browser.active_url(), "https://example.com/");
}

#[test]
fn initial_session_has_one_ready_tab() {
    let browser = browser();
    assert_eq!(browser.session().tabs().len(), 1);
    assert!(browser.session().accepts_commands());
    assert!(browser.active_tab_id().is_some());
}

#[test]
fn stop_command_is_honestly_not_supported() {
    let mut browser = browser();
    let outcome = browser.execute(BrowserCommand::Stop).unwrap();
    assert_eq!(
        outcome,
        CommandOutcome::NotSupported {
            operation: "stop",
            reason: "the platform engine provides no stop() API".to_string(),
        }
    );
}

#[test]
fn chrome_stop_action_is_logged_not_fake_success() {
    let mut browser = browser();
    browser
        .engine_mut()
        .inject_chrome_action(ChromeAction::Stop);
    // Must not panic or invent a load-stop; state stays consistent.
    browser.process_messages().unwrap();
    assert_eq!(browser.session().tabs().len(), 1);
}
