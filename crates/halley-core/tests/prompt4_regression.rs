//! Cross-check: core still drives multi-tab Browser with mock engine
//! after Prompt #4 wiring (regression).

use halley_common::Config;
use halley_core::{Browser, BrowserCommand, CommandOutcome};
use halley_engine::MockEngine;

#[test]
fn multi_tab_regression_still_works() {
    let engine = MockEngine::new();
    let config = Config::default();
    config.validate().expect("default config must validate");
    let mut browser = Browser::new(engine, config, "about:blank").unwrap();
    assert_eq!(browser.session().tabs().len(), 1);

    browser
        .execute(BrowserCommand::NewTab {
            url: Some("https://example.com/".into()),
        })
        .unwrap();
    assert_eq!(browser.session().tabs().len(), 2);

    browser.execute(BrowserCommand::NextTab).unwrap();
    browser.execute(BrowserCommand::PrevTab).unwrap();

    let active = browser.active_tab_id().unwrap();
    browser
        .execute(BrowserCommand::NavigateInput("https://example.org/".into()))
        .unwrap();
    assert_eq!(browser.active_url(), "https://example.org/");

    browser.execute(BrowserCommand::CloseTab(active)).unwrap();
    assert!(!browser.session().tabs().is_empty());
    browser.process_messages().unwrap();
    let _ = CommandOutcome::Done;
}

#[test]
fn profile_opens_beside_browser_without_breaking_tabs() {
    use halley_privacy::BrowserProfile;
    let root = std::env::temp_dir().join(format!("halley-core-profile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut config = Config::default();
    config.storage.profile_root = Some(root.display().to_string());
    config.validate().unwrap();

    let profile = BrowserProfile::open_normal(&config).unwrap();
    let engine = MockEngine::new();
    let mut browser = Browser::new(engine, config, "about:blank").unwrap();
    assert_eq!(browser.session().tabs().len(), 1);
    browser
        .execute(BrowserCommand::NewTab { url: None })
        .unwrap();
    assert_eq!(browser.session().tabs().len(), 2);
    profile.close().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
