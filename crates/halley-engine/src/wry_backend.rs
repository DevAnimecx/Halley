//! Native wry backend: one chrome web view + N content pages.
//!
//! Platform stack (**IMPLEMENTED** on Windows via WebView2; Linux uses
//! WebKitGTK and is compile-tested in CI — see ADR-006):
//!
//! * **Chrome view** — embedded HTML (`assets/chrome.html`), IPC handler,
//!   never navigates off the embedded document.
//! * **Content pages** — one child WebView per [`PageId`]; no IPC handler;
//!   navigation filtered to http/https/about/file; permissions and
//!   downloads denied; new-window requests **denied as native windows**
//!   but reported as [`EngineEvent::NewWindowRequested`] so core can open
//!   a Halley tab (ADR-007).
//!
//! Only the active page is visible; inactive pages stay alive (preserving
//! form/JS state) via [`WebView::set_visible`].

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use raw_window_handle::HasWindowHandle;
use wry::{
    http::Request, NewWindowFeatures, NewWindowResponse, PageLoadEvent, Rect, WebView,
    WebViewBuilder,
};

use crate::chrome_ipc::{chrome_state_script, parse_chrome_message};
use crate::error::EngineError;
use crate::types::{ChromeState, EngineEvent, EngineMessage, PageId};
use crate::BrowserEngine;

/// Embedded browser chrome markup (tab strip + toolbar).
pub const CHROME_HTML: &str = include_str!("../../../assets/chrome.html");

/// Allowlist for content-page navigations (address-bar input is also
/// filtered by core before it reaches the engine).
fn content_navigation_allowed(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("about:")
        || lower.starts_with("file://")
}

fn content_bounds(window_width: f64, window_height: f64, chrome_height: f64) -> Rect {
    let y = chrome_height.max(0.0);
    let h = (window_height - y).max(0.0);
    Rect {
        position: logical_position(0.0, y),
        size: logical_size(window_width.max(0.0), h),
    }
}

fn chrome_bounds(window_width: f64, chrome_height: f64) -> Rect {
    Rect {
        position: logical_position(0.0, 0.0),
        size: logical_size(window_width.max(0.0), chrome_height.max(0.0)),
    }
}

fn logical_position(x: f64, y: f64) -> wry::dpi::Position {
    wry::dpi::LogicalPosition::new(x, y).into()
}

fn logical_size(w: f64, h: f64) -> wry::dpi::Size {
    wry::dpi::LogicalSize::new(w, h).into()
}

struct PageEntry {
    view: WebView,
    url: String,
    title: String,
    loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
}

struct Queue {
    messages: VecDeque<EngineMessage>,
}

/// Multi-page wry backend over a shared host window `W`.
///
/// `W` must outlive the backend (`Application` declares the window before
/// the browser so drop order holds).
pub struct WryBackend<W: HasWindowHandle> {
    window: Arc<W>,
    chrome: WebView,
    pages: HashMap<PageId, PageEntry>,
    next_page_id: PageId,
    active_page: Option<PageId>,
    queue: Arc<Mutex<Queue>>,
    waker: Arc<dyn Fn() + Send + Sync>,
    chrome_height: f64,
    window_width: f64,
    window_height: f64,
    devtools: bool,
}

impl<W: HasWindowHandle + 'static> WryBackend<W> {
    /// Create chrome + layout against `window` (no content pages yet).
    pub fn create(
        window: Arc<W>,
        chrome_height: f64,
        window_width: f64,
        window_height: f64,
        waker: Arc<dyn Fn() + Send + Sync>,
        devtools: bool,
    ) -> Result<Self, EngineError> {
        let queue: Arc<Mutex<Queue>> = Arc::new(Mutex::new(Queue {
            messages: VecDeque::new(),
        }));

        let chrome = {
            let q = Arc::clone(&queue);
            let wake = Arc::clone(&waker);
            WebViewBuilder::new()
                .with_html(CHROME_HTML)
                .with_bounds(chrome_bounds(window_width, chrome_height))
                .with_ipc_handler(move |req: Request<String>| {
                    if let Some(action) = parse_chrome_message(req.body()) {
                        if let Ok(mut guard) = q.lock() {
                            guard.messages.push_back(EngineMessage::Action(action));
                        }
                        wake();
                    }
                })
                .with_navigation_handler(|url| {
                    // Chrome stays on the embedded document only.
                    url.starts_with("data:text/html")
                        || url == "about:blank"
                        || !url.contains(':')
                        || url.starts_with("chrome")
                })
                .with_new_window_req_handler(|_url: String, _features: NewWindowFeatures| {
                    NewWindowResponse::Deny
                })
                .with_permission_handler(|_| wry::PermissionResponse::Deny)
                .with_download_started_handler(|_url, _path| false)
                .with_general_autofill_enabled(false)
                .with_devtools(devtools)
                .build_as_child(window.as_ref())
                .map_err(|err| EngineError::Create(format!("chrome webview: {err}")))?
        };

        Ok(Self {
            window,
            chrome,
            pages: HashMap::new(),
            next_page_id: 1,
            active_page: None,
            queue,
            waker,
            chrome_height,
            window_width,
            window_height,
            devtools,
        })
    }

    fn require_page(&self, page: PageId) -> Result<&PageEntry, EngineError> {
        self.pages.get(&page).ok_or(EngineError::PageNotFound(page))
    }

    fn require_page_mut(&mut self, page: PageId) -> Result<&mut PageEntry, EngineError> {
        self.pages
            .get_mut(&page)
            .ok_or(EngineError::PageNotFound(page))
    }
}

impl<W: HasWindowHandle + 'static> BrowserEngine for WryBackend<W> {
    fn create_page(&mut self, url: &str) -> Result<PageId, EngineError> {
        let page = self.next_page_id;
        self.next_page_id += 1;

        let bounds = content_bounds(self.window_width, self.window_height, self.chrome_height);
        let queue = Arc::clone(&self.queue);
        let waker = Arc::clone(&self.waker);
        let title_queue = Arc::clone(&self.queue);
        let title_waker = Arc::clone(&self.waker);
        let load_queue = Arc::clone(&self.queue);
        let load_waker = Arc::clone(&self.waker);
        let popup_queue = Arc::clone(&self.queue);
        let popup_waker = Arc::clone(&self.waker);
        let title_page = page;
        let load_page = page;
        let popup_page = page;
        let _ = &queue;
        let _ = &waker;

        let view = WebViewBuilder::new()
            .with_url(url)
            .with_bounds(bounds)
            .with_navigation_handler(|nav| content_navigation_allowed(&nav))
            .with_ipc_handler(|_req: Request<String>| {
                // Content pages have no IPC surface — ignore silently.
            })
            .with_on_page_load_handler(move |phase: PageLoadEvent, loaded_url: String| {
                let event = match phase {
                    PageLoadEvent::Started => EngineEvent::PageStarted {
                        page: load_page,
                        url: loaded_url,
                    },
                    PageLoadEvent::Finished => EngineEvent::PageFinished {
                        page: load_page,
                        url: loaded_url,
                    },
                };
                if let Ok(mut guard) = load_queue.lock() {
                    guard.messages.push_back(EngineMessage::Event(event));
                }
                load_waker();
            })
            .with_document_title_changed_handler(move |title: String| {
                if let Ok(mut guard) = title_queue.lock() {
                    guard
                        .messages
                        .push_back(EngineMessage::Event(EngineEvent::TitleChanged {
                            page: title_page,
                            title,
                        }));
                }
                title_waker();
            })
            .with_new_window_req_handler(move |nav_url: String, _features: NewWindowFeatures| {
                if content_navigation_allowed(&nav_url) {
                    if let Ok(mut guard) = popup_queue.lock() {
                        guard.messages.push_back(EngineMessage::Event(
                            EngineEvent::NewWindowRequested {
                                page: popup_page,
                                url: nav_url,
                            },
                        ));
                    }
                    popup_waker();
                }
                NewWindowResponse::Deny
            })
            .with_permission_handler(|_| wry::PermissionResponse::Deny)
            .with_download_started_handler(|_url, _path| false)
            .with_general_autofill_enabled(false)
            .with_devtools(self.devtools)
            .build_as_child(self.window.as_ref())
            .map_err(|err| EngineError::Create(format!("content page: {err}")))?;

        // First page becomes visible; later pages wait until activated.
        let visible = self.active_page.is_none();
        if visible {
            if let Err(err) = view.set_visible(true) {
                return Err(EngineError::Create(format!("show page: {err}")));
            }
            self.active_page = Some(page);
        } else if let Err(err) = view.set_visible(false) {
            return Err(EngineError::Create(format!("hide page: {err}")));
        }

        self.pages.insert(
            page,
            PageEntry {
                view,
                url: url.to_string(),
                title: String::new(),
                loading: true,
                can_go_back: false,
                can_go_forward: false,
            },
        );
        Ok(page)
    }

    fn destroy_page(&mut self, page: PageId) -> Result<(), EngineError> {
        let entry = self
            .pages
            .remove(&page)
            .ok_or(EngineError::PageNotFound(page))?;
        drop(entry.view);
        if self.active_page == Some(page) {
            self.active_page = self.pages.keys().next().copied();
        }
        Ok(())
    }

    fn set_page_visible(&mut self, page: PageId, visible: bool) -> Result<(), EngineError> {
        let entry = self.require_page(page)?;
        entry
            .view
            .set_visible(visible)
            .map_err(|err| EngineError::Layout(err.to_string()))?;
        if visible {
            self.active_page = Some(page);
            let entry = self.require_page(page)?;
            entry
                .view
                .set_bounds(content_bounds(
                    self.window_width,
                    self.window_height,
                    self.chrome_height,
                ))
                .map_err(|err| EngineError::Layout(err.to_string()))?;
            entry
                .view
                .focus()
                .map_err(|err| EngineError::Focus(err.to_string()))?;
        }
        Ok(())
    }

    fn navigate(&mut self, page: PageId, url: &str) -> Result<(), EngineError> {
        if !content_navigation_allowed(url) {
            return Err(EngineError::Navigate(format!("blocked scheme for {url}")));
        }
        let entry = self.require_page_mut(page)?;
        entry
            .view
            .load_url(url)
            .map_err(|err| EngineError::Navigate(err.to_string()))?;
        entry.url = url.to_string();
        entry.loading = true;
        Ok(())
    }

    fn back(&mut self, page: PageId) -> Result<(), EngineError> {
        let entry = self.require_page_mut(page)?;
        if !entry.can_go_back {
            return Err(EngineError::InvalidState("no back entry".into()));
        }
        entry
            .view
            .go_back()
            .map_err(|err| EngineError::Navigate(err.to_string()))?;
        entry.loading = true;
        entry.can_go_back = entry.view.can_go_back().unwrap_or(false);
        entry.can_go_forward = entry.view.can_go_forward().unwrap_or(false);
        if let Ok(url) = entry.view.url() {
            entry.url = url;
        }
        Ok(())
    }

    fn forward(&mut self, page: PageId) -> Result<(), EngineError> {
        let entry = self.require_page_mut(page)?;
        if !entry.can_go_forward {
            return Err(EngineError::InvalidState("no forward entry".into()));
        }
        entry
            .view
            .go_forward()
            .map_err(|err| EngineError::Navigate(err.to_string()))?;
        entry.loading = true;
        entry.can_go_back = entry.view.can_go_back().unwrap_or(false);
        entry.can_go_forward = entry.view.can_go_forward().unwrap_or(false);
        if let Ok(url) = entry.view.url() {
            entry.url = url;
        }
        Ok(())
    }

    fn reload(&mut self, page: PageId) -> Result<(), EngineError> {
        let entry = self.require_page_mut(page)?;
        entry
            .view
            .reload()
            .map_err(|err| EngineError::Navigate(err.to_string()))?;
        entry.loading = true;
        Ok(())
    }

    fn stop(&mut self, _page: PageId) -> Result<(), EngineError> {
        // wry 0.57 has no WebView::stop(); do not fake one.
        Err(EngineError::Unsupported(
            "stop is not provided by the wry WebView",
        ))
    }

    fn can_go_back(&self, page: PageId) -> bool {
        self.pages
            .get(&page)
            .map(|p| p.can_go_back)
            .unwrap_or(false)
    }

    fn can_go_forward(&self, page: PageId) -> bool {
        self.pages
            .get(&page)
            .map(|p| p.can_go_forward)
            .unwrap_or(false)
    }

    fn page_url(&self, page: PageId) -> &str {
        self.pages.get(&page).map(|p| p.url.as_str()).unwrap_or("")
    }

    fn page_title(&self, page: PageId) -> &str {
        self.pages
            .get(&page)
            .map(|p| p.title.as_str())
            .unwrap_or("")
    }

    fn page_is_loading(&self, page: PageId) -> bool {
        self.pages.get(&page).map(|p| p.loading).unwrap_or(false)
    }

    fn update_chrome(&mut self, state: &ChromeState) -> Result<(), EngineError> {
        // Refresh nav flags from the live WebView before pushing.
        let active = self.active_page;
        if let Some(page_id) = active {
            if let Some(entry) = self.pages.get_mut(&page_id) {
                entry.can_go_back = entry.view.can_go_back().unwrap_or(entry.can_go_back);
                entry.can_go_forward = entry.view.can_go_forward().unwrap_or(entry.can_go_forward);
                if let Ok(url) = entry.view.url() {
                    if !url.is_empty() {
                        entry.url = url;
                    }
                }
            }
        }

        let mut snapshot = state.clone();
        if let Some(page_id) = active {
            if let Some(entry) = self.pages.get(&page_id) {
                snapshot.can_go_back = entry.can_go_back;
                snapshot.can_go_forward = entry.can_go_forward;
                if snapshot.url.is_empty() {
                    snapshot.url = entry.url.clone();
                }
            }
        }

        let script = chrome_state_script(&snapshot);
        self.chrome
            .evaluate_script(&script)
            .map_err(|err| EngineError::ChromeUpdate(err.to_string()))
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
        self.window_width = window_width;
        self.window_height = window_height;
        self.chrome_height = chrome_height;

        self.chrome
            .set_bounds(chrome_bounds(window_width, chrome_height))
            .map_err(|err| EngineError::Layout(err.to_string()))?;

        let content = content_bounds(window_width, window_height, chrome_height);
        for entry in self.pages.values() {
            entry
                .view
                .set_bounds(content)
                .map_err(|err| EngineError::Layout(err.to_string()))?;
        }
        Ok(())
    }

    fn drain_messages(&mut self) -> Vec<EngineMessage> {
        let mut out = Vec::new();
        if let Ok(mut guard) = self.queue.lock() {
            out.extend(guard.messages.drain(..));
        }

        // Sync page bookkeeping from load/title events that just arrived.
        for message in &out {
            if let EngineMessage::Event(event) = message {
                match event {
                    EngineEvent::PageStarted { page, url } => {
                        if let Some(entry) = self.pages.get_mut(page) {
                            entry.loading = true;
                            entry.url = url.clone();
                        }
                    }
                    EngineEvent::PageFinished { page, url } => {
                        if let Some(entry) = self.pages.get_mut(page) {
                            entry.loading = false;
                            entry.url = url.clone();
                            entry.can_go_back = entry.view.can_go_back().unwrap_or(false);
                            entry.can_go_forward = entry.view.can_go_forward().unwrap_or(false);
                        }
                    }
                    EngineEvent::TitleChanged { page, title } => {
                        if let Some(entry) = self.pages.get_mut(page) {
                            entry.title = title.clone();
                        }
                    }
                    EngineEvent::NewWindowRequested { .. } => {}
                }
            }
        }
        out
    }

    fn focus_active_page(&mut self) -> Result<(), EngineError> {
        if let Some(page) = self.active_page {
            let entry = self.require_page(page)?;
            entry
                .view
                .focus()
                .map_err(|err| EngineError::Focus(err.to_string()))
        } else {
            Ok(())
        }
    }
}
