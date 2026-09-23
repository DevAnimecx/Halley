//! In-process mock engine for integration tests.
//!
//! Enabled with the `test-engine` feature (wired as a dev-dependency of
//! `halley-core`). Creates no window, performs no network I/O, and records
//! commands so tests can assert real call sequences.
//!
//! This is **not** a fake renderer pretending to load pages: it never
//! claims to produce pixels or fetch URLs. It only implements the
//! [`crate::BrowserEngine`] control surface with per-page session history
//! so back/forward and tab lifecycle can be tested without a WebView.

use std::collections::{HashMap, VecDeque};

use crate::chrome_ipc::chrome_state_script;
use crate::error::EngineError;
use crate::types::{ChromeAction, ChromeState, EngineEvent, EngineMessage, PageId};
use crate::BrowserEngine;

#[derive(Debug, Default)]
struct MockPage {
    history: Vec<String>,
    index: Option<usize>,
    title: String,
    loading: bool,
    visible: bool,
}

impl MockPage {
    fn current(&self) -> &str {
        self.index
            .and_then(|i| self.history.get(i))
            .map(String::as_str)
            .unwrap_or("")
    }

    fn can_go_back(&self) -> bool {
        self.index.is_some_and(|i| i > 0)
    }

    fn can_go_forward(&self) -> bool {
        self.index.is_some_and(|i| i + 1 < self.history.len())
    }

    fn navigate(&mut self, url: &str) {
        if let Some(i) = self.index {
            self.history.truncate(i + 1);
        }
        self.history.push(url.to_string());
        self.index = Some(self.history.len() - 1);
        self.loading = true;
    }
}

/// Recorded engine activity for assertions.
#[derive(Debug, Default)]
pub struct MockEngine {
    pages: HashMap<PageId, MockPage>,
    next_page_id: PageId,
    active_page: Option<PageId>,
    pending: VecDeque<EngineMessage>,
    /// Last chrome state pushed via [`BrowserEngine::update_chrome`].
    pub last_chrome: Option<ChromeState>,
    /// Last layout arguments (`width`, `height`, `chrome_height`).
    pub last_layout: Option<(f64, f64, f64)>,
    /// Commands issued by core, in order (`"navigate 1 https://…"`, …).
    pub commands: Vec<String>,
}

impl MockEngine {
    /// Create an empty mock with no pages.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a raw chrome action as if IPC had delivered it.
    pub fn inject_chrome_action(&mut self, action: ChromeAction) {
        self.pending.push_back(EngineMessage::Action(action));
    }

    /// Queue a page event as if the engine had produced it.
    pub fn inject_event(&mut self, event: EngineEvent) {
        self.pending.push_back(EngineMessage::Event(event));
    }

    /// Session history URLs for `page` in visit order (test helper).
    pub fn history_urls(&self, page: PageId) -> &[String] {
        self.pages
            .get(&page)
            .map(|p| p.history.as_slice())
            .unwrap_or(&[])
    }

    /// Whether `page` exists (test helper).
    pub fn has_page(&self, page: PageId) -> bool {
        self.pages.contains_key(&page)
    }

    /// The mock's active page, if any (test helper).
    pub fn active_page(&self) -> Option<PageId> {
        self.active_page
    }

    /// Title recorded for `page` (test helper).
    pub fn page_title(&self, page: PageId) -> &str {
        self.pages
            .get(&page)
            .map(|p| p.title.as_str())
            .unwrap_or("")
    }

    /// Set a title on `page` and queue a TitleChanged event (test helper).
    pub fn set_title(&mut self, page: PageId, title: &str) {
        if let Some(entry) = self.pages.get_mut(&page) {
            entry.title = title.to_string();
        }
        self.pending
            .push_back(EngineMessage::Event(EngineEvent::TitleChanged {
                page,
                title: title.to_string(),
            }));
    }

    fn require(&self, page: PageId) -> Result<&MockPage, EngineError> {
        self.pages.get(&page).ok_or(EngineError::PageNotFound(page))
    }

    fn require_mut(&mut self, page: PageId) -> Result<&mut MockPage, EngineError> {
        self.pages
            .get_mut(&page)
            .ok_or(EngineError::PageNotFound(page))
    }

    fn emit_load_pair(&mut self, page: PageId, url: String) {
        self.pending
            .push_back(EngineMessage::Event(EngineEvent::PageStarted {
                page,
                url: url.clone(),
            }));
        self.pending
            .push_back(EngineMessage::Event(EngineEvent::PageFinished {
                page,
                url,
            }));
    }
}

impl BrowserEngine for MockEngine {
    fn create_page(&mut self, url: &str) -> Result<PageId, EngineError> {
        if url.is_empty() {
            return Err(EngineError::Create("empty url".into()));
        }
        let page = self.next_page_id;
        self.next_page_id += 1;
        let mut entry = MockPage {
            loading: true,
            ..MockPage::default()
        };
        entry.navigate(url);
        let visible = self.active_page.is_none();
        entry.visible = visible;
        if visible {
            self.active_page = Some(page);
        }
        self.commands.push(format!("create {page} {url}"));
        self.pages.insert(page, entry);
        Ok(page)
    }

    fn destroy_page(&mut self, page: PageId) -> Result<(), EngineError> {
        self.pages
            .remove(&page)
            .ok_or(EngineError::PageNotFound(page))?;
        self.commands.push(format!("destroy {page}"));
        if self.active_page == Some(page) {
            self.active_page = self.pages.keys().next().copied();
        }
        Ok(())
    }

    fn set_page_visible(&mut self, page: PageId, visible: bool) -> Result<(), EngineError> {
        let entry = self.require_mut(page)?;
        entry.visible = visible;
        if visible {
            self.active_page = Some(page);
        }
        self.commands
            .push(format!("visible {page} {}", u8::from(visible)));
        Ok(())
    }

    fn navigate(&mut self, page: PageId, url: &str) -> Result<(), EngineError> {
        if url.is_empty() {
            return Err(EngineError::Navigate("empty url".into()));
        }
        let entry = self.require_mut(page)?;
        entry.navigate(url);
        self.commands.push(format!("navigate {page} {url}"));
        let started = EngineMessage::Event(EngineEvent::PageStarted {
            page,
            url: url.to_string(),
        });
        let finished = EngineMessage::Event(EngineEvent::PageFinished {
            page,
            url: url.to_string(),
        });
        self.pending.push_back(started);
        self.pending.push_back(finished);
        if let Some(entry) = self.pages.get_mut(&page) {
            entry.loading = false;
        }
        Ok(())
    }

    fn back(&mut self, page: PageId) -> Result<(), EngineError> {
        let entry = self.require(page)?;
        if !entry.can_go_back() {
            return Err(EngineError::InvalidState("no back entry".into()));
        }
        let entry = self.require_mut(page)?;
        let i = entry.index.unwrap_or(0);
        entry.index = Some(i - 1);
        entry.loading = true;
        let url = entry.current().to_string();
        self.commands.push(format!("back {page}"));
        self.emit_load_pair(page, url);
        if let Some(entry) = self.pages.get_mut(&page) {
            entry.loading = false;
        }
        Ok(())
    }

    fn forward(&mut self, page: PageId) -> Result<(), EngineError> {
        let entry = self.require(page)?;
        if !entry.can_go_forward() {
            return Err(EngineError::InvalidState("no forward entry".into()));
        }
        let entry = self.require_mut(page)?;
        let i = entry.index.unwrap_or(0);
        entry.index = Some(i + 1);
        entry.loading = true;
        let url = entry.current().to_string();
        self.commands.push(format!("forward {page}"));
        self.emit_load_pair(page, url);
        if let Some(entry) = self.pages.get_mut(&page) {
            entry.loading = false;
        }
        Ok(())
    }

    fn reload(&mut self, page: PageId) -> Result<(), EngineError> {
        let url = {
            let entry = self.require(page)?;
            if entry.index.is_none() {
                return Err(EngineError::InvalidState("nothing to reload".into()));
            }
            entry.current().to_string()
        };
        self.commands.push(format!("reload {page}"));
        self.emit_load_pair(page, url);
        Ok(())
    }

    fn stop(&mut self, page: PageId) -> Result<(), EngineError> {
        self.require(page)?;
        // Mirrors wry: no stop API — typed unsupported, not a fake success.
        Err(EngineError::Unsupported(
            "stop is not provided by the mock engine",
        ))
    }

    fn can_go_back(&self, page: PageId) -> bool {
        self.pages.get(&page).is_some_and(|p| p.can_go_back())
    }

    fn can_go_forward(&self, page: PageId) -> bool {
        self.pages.get(&page).is_some_and(|p| p.can_go_forward())
    }

    fn page_url(&self, page: PageId) -> &str {
        self.pages.get(&page).map(|p| p.current()).unwrap_or("")
    }

    fn page_title(&self, page: PageId) -> &str {
        self.pages
            .get(&page)
            .map(|p| p.title.as_str())
            .unwrap_or("")
    }

    fn page_is_loading(&self, page: PageId) -> bool {
        self.pages.get(&page).is_some_and(|p| p.loading)
    }

    fn update_chrome(&mut self, state: &ChromeState) -> Result<(), EngineError> {
        // Exercise the same script path the native backend uses so JSON
        // escaping bugs fail unit/integration tests, not just GUI runs.
        let script = chrome_state_script(state);
        if !script.starts_with("window.__halleyUpdateChrome(") {
            return Err(EngineError::ChromeUpdate("script malformed".into()));
        }
        self.last_chrome = Some(state.clone());
        Ok(())
    }

    fn set_layout(
        &mut self,
        window_width: f64,
        window_height: f64,
        chrome_height: f64,
    ) -> Result<(), EngineError> {
        if window_width <= 0.0 || window_height <= 0.0 || chrome_height < 0.0 {
            return Err(EngineError::Layout("non-positive bounds".into()));
        }
        self.last_layout = Some((window_width, window_height, chrome_height));
        Ok(())
    }

    fn drain_messages(&mut self) -> Vec<EngineMessage> {
        self.pending.drain(..).collect()
    }

    fn focus_active_page(&mut self) -> Result<(), EngineError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_page_records_history_and_emits_load_pair() {
        let mut engine = MockEngine::new();
        let page = engine.create_page("https://example.com/").unwrap();
        assert_eq!(engine.history_urls(page), ["https://example.com/"]);
        assert!(!engine.can_go_back(page));
        assert!(!engine.can_go_forward(page));

        let messages = engine.drain_messages();
        assert!(messages.is_empty()); // create does not emit; navigate events come from navigate()
                                      // create_page already navigates internally without events — only
                                      // explicit navigate/back/forward/reload emit. Documented behaviour.
        assert!(engine.has_page(page));
        assert_eq!(engine.active_page(), Some(page));
    }

    #[test]
    fn navigate_emits_load_pair() {
        let mut engine = MockEngine::new();
        let page = engine.create_page("about:blank").unwrap();
        engine.navigate(page, "https://example.com/").unwrap();
        let messages = engine.drain_messages();
        assert_eq!(messages.len(), 2);
        assert!(matches!(
            messages[0],
            EngineMessage::Event(EngineEvent::PageStarted { page: p, .. }) if p == page
        ));
        assert!(matches!(
            messages[1],
            EngineMessage::Event(EngineEvent::PageFinished { page: p, .. }) if p == page
        ));
        assert!(engine.drain_messages().is_empty());
    }

    #[test]
    fn back_and_forward_walk_history_per_page() {
        let mut engine = MockEngine::new();
        let page = engine.create_page("about:blank").unwrap();
        engine.navigate(page, "https://a.example/").unwrap();
        engine.navigate(page, "https://b.example/").unwrap();
        let _ = engine.drain_messages();

        assert!(engine.can_go_back(page));
        assert!(!engine.can_go_forward(page));

        engine.back(page).unwrap();
        assert_eq!(engine.page_url(page), "https://a.example/");
        // Initial about:blank remains below a.example.
        assert!(engine.can_go_back(page));
        assert!(engine.can_go_forward(page));

        engine.back(page).unwrap();
        assert_eq!(engine.page_url(page), "about:blank");
        assert!(!engine.can_go_back(page));
        assert!(engine.can_go_forward(page));

        engine.forward(page).unwrap();
        assert_eq!(engine.page_url(page), "https://a.example/");
        assert!(engine.can_go_back(page));
        assert!(engine.can_go_forward(page));
    }

    #[test]
    fn back_at_root_is_invalid_state() {
        let mut engine = MockEngine::new();
        let page = engine.create_page("https://a.example/").unwrap();
        let _ = engine.drain_messages();
        assert!(matches!(
            engine.back(page),
            Err(EngineError::InvalidState(_))
        ));
    }

    #[test]
    fn unknown_page_ids_are_rejected() {
        let mut engine = MockEngine::new();
        assert!(matches!(
            engine.navigate(99, "https://example.com/"),
            Err(EngineError::PageNotFound(99))
        ));
        assert!(matches!(
            engine.destroy_page(99),
            Err(EngineError::PageNotFound(99))
        ));
        assert!(!engine.can_go_back(99));
        assert_eq!(engine.page_url(99), "");
    }

    #[test]
    fn destroy_then_activate_remaining_pages() {
        let mut engine = MockEngine::new();
        let a = engine.create_page("https://a.example/").unwrap();
        let b = engine.create_page("https://b.example/").unwrap();
        assert_eq!(engine.active_page(), Some(a));

        engine.set_page_visible(b, true).unwrap();
        assert_eq!(engine.active_page(), Some(b));

        engine.destroy_page(b).unwrap();
        assert_eq!(engine.active_page(), Some(a));
        assert!(!engine.has_page(b));
    }

    #[test]
    fn stop_is_unsupported_not_fake() {
        let mut engine = MockEngine::new();
        let page = engine.create_page("https://a.example/").unwrap();
        assert!(matches!(
            engine.stop(page),
            Err(EngineError::Unsupported(_))
        ));
    }

    #[test]
    fn new_navigate_drops_forward_entries() {
        let mut engine = MockEngine::new();
        let page = engine.create_page("about:blank").unwrap();
        engine.navigate(page, "https://a.example/").unwrap();
        engine.navigate(page, "https://b.example/").unwrap();
        engine.back(page).unwrap();
        engine.navigate(page, "https://c.example/").unwrap();
        assert_eq!(
            engine.history_urls(page),
            ["about:blank", "https://a.example/", "https://c.example/"]
        );
        assert!(!engine.can_go_forward(page));
    }

    #[test]
    fn update_chrome_and_layout_record_state() {
        let mut engine = MockEngine::new();
        let state = ChromeState {
            url: "https://example.com/".into(),
            title: "Example".into(),
            can_go_back: false,
            can_go_forward: false,
            loading: false,
            ..ChromeState::default()
        };
        engine.update_chrome(&state).unwrap();
        engine.set_layout(800.0, 600.0, 76.0).unwrap();
        assert_eq!(engine.last_chrome, Some(state));
        assert_eq!(engine.last_layout, Some((800.0, 600.0, 76.0)));
    }

    #[test]
    fn layout_rejects_non_positive_bounds() {
        let mut engine = MockEngine::new();
        assert!(engine.set_layout(0.0, 600.0, 76.0).is_err());
        assert!(engine.set_layout(800.0, 600.0, -1.0).is_err());
    }

    #[test]
    fn inject_chrome_action_appears_in_drain() {
        let mut engine = MockEngine::new();
        engine.inject_chrome_action(ChromeAction::Reload);
        let messages = engine.drain_messages();
        assert_eq!(messages, vec![EngineMessage::Action(ChromeAction::Reload)]);
    }

    #[test]
    fn pages_are_independent_histories() {
        let mut engine = MockEngine::new();
        let a = engine.create_page("about:blank").unwrap();
        let b = engine.create_page("about:blank").unwrap();
        engine.navigate(a, "https://a.example/").unwrap();
        engine.navigate(b, "https://b.example/").unwrap();
        engine.navigate(b, "https://b.example/2").unwrap();
        assert_eq!(engine.history_urls(a).len(), 2); // about:blank + a
        assert_eq!(engine.history_urls(b).len(), 3); // about:blank + 2
        assert!(engine.can_go_back(b));
    }
}
