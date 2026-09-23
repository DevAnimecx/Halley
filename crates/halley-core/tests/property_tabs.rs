//! Property-style tests: tab strip invariants under random command
//! sequences (no proptest dependency — deterministic LCG).
//!
//! No network I/O.

use halley_common::Config;
use halley_core::{Browser, BrowserCommand, TabId};
use halley_engine::MockEngine;

/// 32-bit LCG (Numerical Recipes constants) — deterministic, no deps.
struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0
    }

    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

fn browser() -> Browser<MockEngine> {
    Browser::new(MockEngine::new(), Config::default(), "about:blank").expect("init")
}

fn assert_strip_invariants(browser: &Browser<MockEngine>) {
    let session = browser.session();
    let tabs = session.tabs();
    tabs.assert_invariants();
    // Production invariant: never an empty strip with a live session.
    assert!(
        !tabs.is_empty(),
        "strip must retain at least one tab (closed {tabs:?})"
    );
    assert!(tabs.active_id().is_some(), "active must always be set");
    // 1:1 tab↔page for every open tab.
    for tab in tabs.tabs() {
        assert!(
            browser.engine().has_page(tab.page()),
            "tab {} has no engine page {}",
            tab.id().raw(),
            tab.page()
        );
    }
}

fn random_live_tab(browser: &Browser<MockEngine>, rng: &mut Lcg) -> TabId {
    let tabs = browser.session().tabs().tabs();
    let idx = rng.below(tabs.len() as u32) as usize;
    tabs[idx].id()
}

#[test]
fn random_open_close_activate_move_preserves_invariants() {
    let mut rng = Lcg(0xC0FFEE);
    let mut browser = browser();
    assert_strip_invariants(&browser);

    for step in 0..500 {
        let choice = rng.below(7);
        let result = match choice {
            0 => browser.execute(BrowserCommand::NewTab { url: None }),
            1 => {
                let id = random_live_tab(&browser, &mut rng);
                browser.execute(BrowserCommand::CloseTab(id))
            }
            2 => {
                let id = random_live_tab(&browser, &mut rng);
                browser.execute(BrowserCommand::ActivateTab(id))
            }
            3 => browser.execute(BrowserCommand::NextTab),
            4 => browser.execute(BrowserCommand::PrevTab),
            5 => browser.execute(BrowserCommand::ReopenClosedTab),
            _ => {
                let id = random_live_tab(&browser, &mut rng);
                let to = rng.below(8) as usize;
                browser.execute(BrowserCommand::MoveTab {
                    tab_id: id,
                    to_index: to,
                })
            }
        };
        result.unwrap_or_else(|err| panic!("step {step} failed: {err}"));
        assert_strip_invariants(&browser);

        // Keep the strip from growing without bound during the run.
        if browser.session().tabs().len() > 20 {
            let id = random_live_tab(&browser, &mut rng);
            browser.execute(BrowserCommand::CloseTab(id)).unwrap();
            assert_strip_invariants(&browser);
        }
    }
}

#[test]
fn close_others_and_to_right_preserve_invariants() {
    let mut rng = Lcg(42);
    let mut browser = browser();
    for _ in 0..6 {
        browser
            .execute(BrowserCommand::NewTab { url: None })
            .unwrap();
    }
    for step in 0..100 {
        let id = random_live_tab(&browser, &mut rng);
        let cmd = if rng.below(2) == 0 {
            BrowserCommand::CloseOtherTabs(id)
        } else {
            BrowserCommand::CloseTabsToRight(id)
        };
        browser.execute(cmd).unwrap();
        assert_strip_invariants(&browser);
        while browser.session().tabs().len() < 4 {
            browser
                .execute(BrowserCommand::NewTab { url: None })
                .unwrap();
        }
        let _ = step;
    }
}

#[test]
fn closed_stack_never_exceeds_configured_bound() {
    let mut config = Config::default();
    config.browser.max_closed_tabs = 5;
    let mut browser = Browser::new(MockEngine::new(), config, "about:blank").unwrap();
    for i in 0..30 {
        browser
            .execute(BrowserCommand::NewTab { url: None })
            .unwrap();
        let id = browser.active_tab_id().unwrap();
        let _ = i;
        // Close a non-last tab when possible to grow the stack.
        if browser.session().tabs().len() > 1 {
            browser.execute(BrowserCommand::CloseTab(id)).unwrap();
        }
        assert!(browser.session().tabs().closed_len() <= 5);
    }
}
