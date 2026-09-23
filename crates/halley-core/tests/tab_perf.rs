//! Multi-tab scale smoke (mock engine, no network, no window).
//!
//! Documents rough orchestration cost for 1/5/10/20 open tabs when
//! creating, activating, and tearing down tabs through the real
//! `Browser` command path. These are wall-clock sanity checks with
//! generous budgets — not microbenchmarks.
//!
//! Real WebView memory per tab (PRD `<30 MB/tab`) is **not measured
//! here**; see `docs/performance-baseline.md` for the manual procedure.

use std::time::Instant;

use halley_common::Config;
use halley_core::{Browser, BrowserCommand};
use halley_engine::MockEngine;

fn browser() -> Browser<MockEngine> {
    Browser::new(MockEngine::new(), Config::default(), "about:blank").expect("init")
}

fn open_n_and_measure(n: usize) -> (std::time::Duration, std::time::Duration) {
    let mut browser = browser();
    let open_start = Instant::now();
    for _ in 1..n {
        browser
            .execute(BrowserCommand::NewTab { url: None })
            .unwrap();
    }
    let open_elapsed = open_start.elapsed();
    assert_eq!(browser.session().tabs().len(), n);

    let close_start = Instant::now();
    // Close every other tab down to a single tab (never empty).
    while browser.session().tabs().len() > 1 {
        let id = browser.session().tabs().tabs()[0].id();
        browser.execute(BrowserCommand::CloseTab(id)).unwrap();
    }
    let close_elapsed = close_start.elapsed();
    assert_eq!(browser.session().tabs().len(), 1);
    (open_elapsed, close_elapsed)
}

#[test]
fn open_close_costs_stay_reasonable_for_1_5_10_20_tabs() {
    for n in [1usize, 5, 10, 20] {
        let (open, close) = open_n_and_measure(n);
        // Generous CI budgets: pure in-memory orchestration should be
        // well under this even on slow runners.
        assert!(open.as_millis() < 2_000, "opening {n} tabs took {open:?}");
        assert!(
            close.as_millis() < 2_000,
            "closing down to 1 of {n} tabs took {close:?}"
        );
        eprintln!("tabs={n} open={open:?} close_to_1={close:?}");
    }
}

#[test]
fn activation_walk_over_many_tabs_is_fast() {
    let mut browser = browser();
    for _ in 1..20 {
        browser
            .execute(BrowserCommand::NewTab { url: None })
            .unwrap();
    }
    let start = Instant::now();
    for _ in 0..100 {
        browser.execute(BrowserCommand::NextTab).unwrap();
    }
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() < 2_000, "100 next-tab ops: {elapsed:?}");
    assert_eq!(browser.session().tabs().len(), 20);
}
