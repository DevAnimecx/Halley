//! Multi-tab browser controller: session + tabs + engine orchestration.
//!
//! [`Browser`] is engine-agnostic. It owns the authoritative
//! [`BrowserSession`] the chrome renders, maps chrome actions and engine
//! events onto typed [`BrowserCommand`]s, and keeps engine pages in
//! lockstep with tabs. The native event loop lives in
//! [`crate::Application`]; tests drive `Browser` with
//! `halley_engine::MockEngine`.
//!
//! Invariants:
//! * There is always at least one open tab after construction (closing
//!   the last tab creates a fresh `new_tab_url` tab).
//! * Exactly one tab is active whenever the tab list is non-empty.
//! * Engine pages exist 1:1 with open tabs.

use halley_common::Config;
use halley_engine::{
    BrowserEngine, ChromeAction, ChromeState, ChromeTabState, EngineError, EngineEvent,
    EngineMessage, JerryChromeState, PageId,
};

use crate::command::{BrowserCommand, CommandOutcome};
use crate::error::CoreError;
use crate::event::BrowserEvent;
use crate::navigation::normalize_address;
use crate::session::{BrowserSession, SessionLifecycle};
use crate::tab::{truncate_title, Tab, TabId};

/// Multi-tab browser: engine handle + authoritative session.
pub struct Browser<E: BrowserEngine> {
    engine: E,
    session: BrowserSession,
    config: Config,
    /// Set when any chrome-visible field changed since the last push.
    chrome_dirty: bool,
    /// Events produced by the last [`Browser::process_messages`] /
    /// command (drained by the application loop).
    pending_events: Vec<BrowserEvent>,
    /// Jerry chat panel chrome state (updated by `JerryHost` via
    /// [`Browser::set_jerry_state`]).
    jerry: JerryChromeState,
}

impl<E: BrowserEngine> Browser<E> {
    /// Wrap an engine that has already been created against a window
    /// (native) or constructed empty (mock), and open the initial tab.
    ///
    /// `initial_url` must already be absolute (callers normalize CLI /
    /// homepage input).
    pub fn new(engine: E, config: Config, initial_url: &str) -> Result<Self, CoreError> {
        let max_closed = config.browser.max_closed_tabs;
        let mut browser = Self {
            engine,
            session: BrowserSession::new(max_closed),
            config,
            chrome_dirty: true,
            pending_events: Vec::new(),
            jerry: JerryChromeState::default(),
        };

        browser
            .session
            .set_lifecycle(SessionLifecycle::Initializing);
        browser.open_tab(Some(initial_url.to_string()))?;
        browser.session.set_lifecycle(SessionLifecycle::Ready);
        browser
            .pending_events
            .push(BrowserEvent::SessionLifecycleChanged {
                from: SessionLifecycle::Created,
                to: SessionLifecycle::Ready,
            });
        Ok(browser)
    }

    /// Immutable view of the session (tabs, lifecycle, id).
    pub fn session(&self) -> &BrowserSession {
        &self.session
    }

    /// Mutable session for tests / application helpers.
    pub fn session_mut(&mut self) -> &mut BrowserSession {
        &mut self.session
    }

    /// The engine behind this browser (tests and application layout).
    pub fn engine(&self) -> &E {
        &self.engine
    }

    /// Mutable engine access for layout calls from the event loop.
    pub fn engine_mut(&mut self) -> &mut E {
        &mut self.engine
    }

    /// Configuration this browser was constructed with.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Active tab id (always `Some` after successful construction).
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.session.tabs().active_id()
    }

    /// Active tab reference.
    pub fn active_tab(&self) -> Option<&Tab> {
        self.session.tabs().active()
    }

    /// URL for the active tab (address-bar value).
    pub fn active_url(&self) -> &str {
        self.active_tab().map(Tab::url).unwrap_or("")
    }

    /// Title for the active tab (window title source).
    pub fn active_title(&self) -> &str {
        self.active_tab().map(Tab::display_title).unwrap_or("")
    }

    /// Whether the active tab is loading.
    pub fn active_loading(&self) -> bool {
        self.active_tab().is_some_and(Tab::loading)
    }

    /// Whether back navigation is possible on the active tab.
    pub fn can_go_back(&self) -> bool {
        self.active_tab()
            .map(|t| self.engine.can_go_back(t.page()))
            .unwrap_or(false)
    }

    /// Whether forward navigation is possible on the active tab.
    pub fn can_go_forward(&self) -> bool {
        self.active_tab()
            .map(|t| self.engine.can_go_forward(t.page()))
            .unwrap_or(false)
    }

    /// Drain events emitted since the last call (application loop).
    pub fn drain_events(&mut self) -> Vec<BrowserEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Apply Jerry panel state from `JerryHost` and mark chrome dirty.
    pub fn set_jerry_state(&mut self, jerry: JerryChromeState) {
        self.jerry = jerry;
        self.chrome_dirty = true;
    }

    /// Current Jerry panel chrome state.
    pub fn jerry_state(&self) -> &JerryChromeState {
        &self.jerry
    }

    /// Effective chrome height (toolbar + optional Jerry panel).
    pub fn chrome_height(&self) -> u32 {
        let base = self.config.window.chrome_height;
        if self.jerry.open {
            base.saturating_add(self.config.jerry.panel_height)
        } else {
            base
        }
    }

    /// Dispatch a typed command (shared path for chrome + tests + future MCP).
    pub fn execute(&mut self, command: BrowserCommand) -> Result<CommandOutcome, CoreError> {
        if !self.session.accepts_commands() {
            return Err(CoreError::Session(format!(
                "command {:?} rejected in lifecycle {:?}",
                command_short(&command),
                self.session.lifecycle()
            )));
        }
        self.dispatch(command)
    }

    /// Navigate to raw address-bar / CLI input on the **active** tab
    /// (normalized here).
    pub fn navigate_input(&mut self, input: &str) -> Result<(), CoreError> {
        let url = normalize_address(input, &self.config.browser.search_provider)?;
        self.navigate_active_url(&url)
    }

    /// Navigate the active tab to an already-valid absolute URL.
    pub fn navigate_url(&mut self, url: &str) -> Result<(), CoreError> {
        self.navigate_active_url(url)
    }

    /// History back on the active tab.
    pub fn go_back(&mut self) -> Result<(), CoreError> {
        let page = self.require_active_page()?;
        self.engine.back(page)?;
        if let Some(tab) = self.session.tabs_mut().active_mut() {
            tab.set_loading(true);
        }
        self.chrome_dirty = true;
        Ok(())
    }

    /// History forward on the active tab.
    pub fn go_forward(&mut self) -> Result<(), CoreError> {
        let page = self.require_active_page()?;
        self.engine.forward(page)?;
        if let Some(tab) = self.session.tabs_mut().active_mut() {
            tab.set_loading(true);
        }
        self.chrome_dirty = true;
        Ok(())
    }

    /// Reload the active tab.
    pub fn reload(&mut self) -> Result<(), CoreError> {
        let page = self.require_active_page()?;
        self.engine.reload(page)?;
        if let Some(tab) = self.session.tabs_mut().active_mut() {
            tab.set_loading(true);
        }
        self.chrome_dirty = true;
        Ok(())
    }

    /// Open a new tab (`url: None` → `config.browser.new_tab_url`).
    ///
    /// The new tab becomes active. Returns its id.
    pub fn open_tab(&mut self, url: Option<String>) -> Result<TabId, CoreError> {
        let raw = url.unwrap_or_else(|| self.config.browser.new_tab_url.clone());
        let url = normalize_address(&raw, &self.config.browser.search_provider)
            .map_err(|err| CoreError::Config(format!("new tab url {raw:?}: {err}")))?;
        let page = self.engine.create_page(&url)?;
        let id = self.session.tabs_mut().alloc_id();
        // Ensure this page is the visible one.
        self.engine.set_page_visible(page, true)?;
        if let Some(previous) = self
            .session
            .tabs()
            .active()
            .filter(|t| t.page() != page)
            .map(|t| t.page())
        {
            let _ = self.engine.set_page_visible(previous, false);
        }
        let mut tab = Tab::new(id, page, url);
        tab.set_loading(self.engine.page_is_loading(page));
        let title = self.engine.page_title(page).to_string();
        if !title.is_empty() {
            tab.set_title(title);
        }
        self.session.insert_tab(tab);
        self.chrome_dirty = true;
        self.pending_events.push(BrowserEvent::TabCreated(id));
        self.pending_events.push(BrowserEvent::TabActivated(id));
        Ok(id)
    }

    /// Close a tab. Closing the **last** tab opens a fresh
    /// `new_tab_url` tab (never an empty strip).
    pub fn close_tab(&mut self, id: TabId) -> Result<CommandOutcome, CoreError> {
        if self.session.tabs().get(id).is_none() {
            return Ok(CommandOutcome::Noop);
        }
        let page = self
            .session
            .tabs()
            .get(id)
            .map(Tab::page)
            .ok_or_else(|| CoreError::Tab(format!("missing tab {id:?}")))?;

        let was_active = self.session.tabs().active_id() == Some(id);
        let closed_count_before = self.session.tabs().len();

        self.session.tabs_mut().close(id);
        // Tab is out of the manager; destroy its engine page.
        self.engine.destroy_page(page)?;
        self.pending_events.push(BrowserEvent::TabClosed(id));

        if closed_count_before == 1 {
            // Last tab closed — invariant: never leave an empty strip.
            self.open_tab(None)?;
        } else if was_active {
            if let Some(new_active) = self.session.tabs().active_id() {
                self.activate_page_for(new_active)?;
                self.pending_events
                    .push(BrowserEvent::TabActivated(new_active));
            }
        }
        self.chrome_dirty = true;
        Ok(CommandOutcome::Done)
    }

    /// Activate a tab by id and show its page.
    pub fn activate_tab(&mut self, id: TabId) -> Result<CommandOutcome, CoreError> {
        if self.session.tabs().get(id).is_none() {
            return Ok(CommandOutcome::Noop);
        }
        if !self.session.tabs_mut().activate(id) {
            return Ok(CommandOutcome::Noop);
        }
        self.activate_page_for(id)?;
        self.chrome_dirty = true;
        self.pending_events.push(BrowserEvent::TabActivated(id));
        Ok(CommandOutcome::Done)
    }

    /// Activate next tab (wraps).
    pub fn activate_next_tab(&mut self) -> Result<CommandOutcome, CoreError> {
        self.activate_relative(true)
    }

    /// Activate previous tab (wraps).
    pub fn activate_prev_tab(&mut self) -> Result<CommandOutcome, CoreError> {
        self.activate_relative(false)
    }

    /// Reopen the most recently closed tab (no-op if empty stack).
    pub fn reopen_closed_tab(&mut self) -> Result<CommandOutcome, CoreError> {
        let Some(closed) = self.session.tabs_mut().pop_closed() else {
            return Ok(CommandOutcome::Noop);
        };
        // open_tab already emits TabCreated/TabActivated.
        self.open_tab(Some(closed.url))?;
        Ok(CommandOutcome::Done)
    }

    /// Duplicate `source` into a new active tab with the same URL.
    pub fn duplicate_tab(&mut self, source: TabId) -> Result<CommandOutcome, CoreError> {
        let url = self
            .session
            .tabs()
            .get(source)
            .map(|t| t.url().to_string())
            .ok_or_else(|| CoreError::Tab(format!("missing tab {source:?}")))?;
        self.open_tab(Some(url))?;
        Ok(CommandOutcome::Done)
    }

    /// Close every tab except `keep`.
    pub fn close_other_tabs(&mut self, keep: TabId) -> Result<CommandOutcome, CoreError> {
        if self.session.tabs().get(keep).is_none() {
            return Ok(CommandOutcome::Noop);
        }
        let doomed: Vec<(TabId, PageId)> = self
            .session
            .tabs()
            .tabs()
            .iter()
            .filter(|t| t.id() != keep)
            .map(|t| (t.id(), t.page()))
            .collect();
        let removed = self.session.tabs_mut().close_others(keep);
        for (id, page) in doomed {
            self.engine.destroy_page(page)?;
            self.pending_events.push(BrowserEvent::TabClosed(id));
        }
        self.activate_page_for(keep)?;
        self.chrome_dirty = true;
        Ok(if removed.is_empty() {
            CommandOutcome::Noop
        } else {
            CommandOutcome::Done
        })
    }

    /// Close tabs strictly to the right of `from`.
    pub fn close_tabs_to_right(&mut self, from: TabId) -> Result<CommandOutcome, CoreError> {
        let Some(start) = self.session.tabs().index_of(from) else {
            return Ok(CommandOutcome::Noop);
        };
        let doomed: Vec<(TabId, PageId)> = self
            .session
            .tabs()
            .tabs()
            .iter()
            .skip(start + 1)
            .map(|t| (t.id(), t.page()))
            .collect();
        let removed = self.session.tabs_mut().close_to_right(from);
        for (id, page) in doomed {
            self.engine.destroy_page(page)?;
            self.pending_events.push(BrowserEvent::TabClosed(id));
        }
        // Active may have been removed — re-sync visibility.
        if let Some(active) = self.session.tabs().active_id() {
            self.activate_page_for(active)?;
        }
        self.chrome_dirty = true;
        Ok(if removed.is_empty() {
            CommandOutcome::Noop
        } else {
            CommandOutcome::Done
        })
    }

    /// Move a tab within the strip (`to_index` clamped).
    pub fn move_tab(
        &mut self,
        tab_id: TabId,
        to_index: usize,
    ) -> Result<CommandOutcome, CoreError> {
        let Some(from) = self.session.tabs().index_of(tab_id) else {
            return Ok(CommandOutcome::Noop);
        };
        let Some(to) = self.session.tabs_mut().move_tab(tab_id, to_index) else {
            return Ok(CommandOutcome::Noop);
        };
        self.chrome_dirty = true;
        self.pending_events
            .push(BrowserEvent::TabMoved { from, to });
        Ok(CommandOutcome::Done)
    }

    /// Drain engine messages and apply them to state / commands.
    ///
    /// Returns `Err` only for hard engine failures; a back action with no
    /// history is logged and ignored (the chrome button should have been
    /// disabled, but a race is not fatal).
    pub fn process_messages(&mut self) -> Result<(), CoreError> {
        if !self.session.accepts_commands() {
            // Still drain so the queue does not grow during shutdown.
            let _ = self.engine.drain_messages();
            return Ok(());
        }
        let messages = self.engine.drain_messages();
        for message in messages {
            match message {
                EngineMessage::Event(event) => self.apply_engine_event(event),
                EngineMessage::Action(action) => self.apply_chrome_action(action)?,
            }
        }
        if self.chrome_dirty {
            self.push_chrome()?;
        }
        Ok(())
    }

    /// Push current state into the chrome view (always, not only when
    /// dirty — used after construction and on demand).
    pub fn push_chrome(&mut self) -> Result<(), CoreError> {
        let snapshot = self.chrome_snapshot();
        self.engine.update_chrome(&snapshot)?;
        self.chrome_dirty = false;
        Ok(())
    }

    /// Re-layout chrome + all content pages for a window resize
    /// (logical pixels). Chrome height includes the Jerry panel when open.
    pub fn set_layout(&mut self, window_width: f64, window_height: f64) -> Result<(), CoreError> {
        let chrome_height = f64::from(self.chrome_height());
        self.engine
            .set_layout(window_width, window_height, chrome_height)?;
        Ok(())
    }

    /// Begin teardown (lifecycle → Closing; application drops after).
    pub fn begin_close(&mut self) {
        let from = self.session.set_lifecycle(SessionLifecycle::Closing);
        self.pending_events
            .push(BrowserEvent::SessionLifecycleChanged {
                from,
                to: SessionLifecycle::Closing,
            });
    }

    /// Build the chrome-facing snapshot from session + engine flags.
    pub fn chrome_snapshot(&mut self) -> ChromeState {
        // Refresh nav flags from the engine for the active page.
        if let Some(tab) = self.session.tabs_mut().active_mut() {
            let page = tab.page();
            let loading = self.engine.page_is_loading(page);
            let url = self.engine.page_url(page);
            tab.set_loading(loading);
            if !url.is_empty() {
                tab.set_url(url);
            }
            let title = self.engine.page_title(page);
            if !title.is_empty() {
                tab.set_title(title);
            }
        }

        let active = self.session.tabs().active();
        let active_id = self.session.tabs().active_id();
        let tabs: Vec<ChromeTabState> = self
            .session
            .tabs()
            .tabs()
            .iter()
            .map(|t| ChromeTabState {
                id: t.id().to_ipc_string(),
                title: truncate_title(t.display_title()),
                active: active_id == Some(t.id()),
                loading: t.loading(),
            })
            .collect();

        let (url, title, can_go_back, can_go_forward, loading) = match active {
            Some(tab) => {
                let page = tab.page();
                (
                    tab.url().to_string(),
                    truncate_title(tab.display_title()),
                    self.engine.can_go_back(page),
                    self.engine.can_go_forward(page),
                    tab.loading(),
                )
            }
            None => (String::new(), String::new(), false, false, false),
        };

        ChromeState {
            url,
            title,
            can_go_back,
            can_go_forward,
            loading,
            tabs,
            active_tab_id: active_id.map(|id| id.to_ipc_string()).unwrap_or_default(),
            jerry: self.jerry.clone(),
        }
    }

    fn activate_relative(&mut self, next: bool) -> Result<CommandOutcome, CoreError> {
        let activated = if next {
            self.session.tabs_mut().activate_next()
        } else {
            self.session.tabs_mut().activate_prev()
        };
        match activated {
            Some(id) => {
                self.activate_page_for(id)?;
                self.chrome_dirty = true;
                self.pending_events.push(BrowserEvent::TabActivated(id));
                Ok(CommandOutcome::Done)
            }
            None => Ok(CommandOutcome::Noop),
        }
    }

    fn activate_page_for(&mut self, id: TabId) -> Result<(), CoreError> {
        let target: PageId = self
            .session
            .tabs()
            .get(id)
            .map(Tab::page)
            .ok_or_else(|| CoreError::Tab(format!("missing tab {id:?}")))?;
        let others: Vec<PageId> = self
            .session
            .tabs()
            .tabs()
            .iter()
            .map(Tab::page)
            .filter(|p| *p != target)
            .collect();
        for page in others {
            let _ = self.engine.set_page_visible(page, false);
        }
        self.engine.set_page_visible(target, true)?;
        Ok(())
    }

    fn require_active_page(&self) -> Result<PageId, CoreError> {
        self.active_tab()
            .map(Tab::page)
            .ok_or_else(|| CoreError::Session("no active tab".into()))
    }

    fn navigate_active_url(&mut self, url: &str) -> Result<(), CoreError> {
        let page = self.require_active_page()?;
        self.engine.navigate(page, url)?;
        let tab_id = self.active_tab_id().unwrap_or_else(|| TabId::from_raw(0));
        if let Some(tab) = self.session.tabs_mut().active_mut() {
            tab.set_url(url);
            tab.set_loading(true);
        }
        self.chrome_dirty = true;
        self.pending_events.push(BrowserEvent::PageLoadStarted {
            tab: tab_id,
            url: url.to_string(),
        });
        Ok(())
    }

    fn dispatch(&mut self, command: BrowserCommand) -> Result<CommandOutcome, CoreError> {
        match command {
            BrowserCommand::NewTab { url } => {
                self.open_tab(url)?;
                Ok(CommandOutcome::Done)
            }
            BrowserCommand::CloseTab(id) => self.close_tab(id),
            BrowserCommand::ActivateTab(id) => self.activate_tab(id),
            BrowserCommand::NextTab => self.activate_next_tab(),
            BrowserCommand::PrevTab => self.activate_prev_tab(),
            BrowserCommand::ReopenClosedTab => self.reopen_closed_tab(),
            BrowserCommand::DuplicateTab(id) => self.duplicate_tab(id),
            BrowserCommand::CloseOtherTabs(id) => self.close_other_tabs(id),
            BrowserCommand::CloseTabsToRight(id) => self.close_tabs_to_right(id),
            BrowserCommand::MoveTab { tab_id, to_index } => self.move_tab(tab_id, to_index),
            BrowserCommand::NavigateInput(input) => {
                self.navigate_input(&input)?;
                Ok(CommandOutcome::Done)
            }
            BrowserCommand::NavigateUrl(url) => {
                self.navigate_url(&url)?;
                Ok(CommandOutcome::Done)
            }
            BrowserCommand::Back => match self.go_back() {
                Ok(()) => Ok(CommandOutcome::Done),
                Err(CoreError::Engine(EngineError::InvalidState(_))) => {
                    halley_common::log_debug!("back ignored: no history entry");
                    self.chrome_dirty = true;
                    Ok(CommandOutcome::Noop)
                }
                Err(err) => Err(err),
            },
            BrowserCommand::Forward => match self.go_forward() {
                Ok(()) => Ok(CommandOutcome::Done),
                Err(CoreError::Engine(EngineError::InvalidState(_))) => {
                    halley_common::log_debug!("forward ignored: no history entry");
                    self.chrome_dirty = true;
                    Ok(CommandOutcome::Noop)
                }
                Err(err) => Err(err),
            },
            BrowserCommand::Reload => {
                self.reload()?;
                Ok(CommandOutcome::Done)
            }
            BrowserCommand::Stop => Ok(CommandOutcome::NotSupported {
                operation: "stop",
                reason: "the platform engine provides no stop() API".to_string(),
            }),
            BrowserCommand::FocusAddressBar => {
                // Chrome-side focus; core only marks the chrome dirty so
                // the UI can re-sync. Native focus is handled by the
                // chrome page itself (window.__halleyFocusAddress).
                self.chrome_dirty = true;
                Ok(CommandOutcome::Done)
            }
        }
    }

    fn apply_engine_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::PageStarted { page, url } => {
                let update = self.tab_by_page_mut(page).map(|tab| {
                    tab.set_url(url.clone());
                    tab.set_loading(true);
                    tab.id()
                });
                if let Some(id) = update {
                    self.chrome_dirty = true;
                    self.pending_events
                        .push(BrowserEvent::PageLoadStarted { tab: id, url });
                }
            }
            EngineEvent::PageFinished { page, url } => {
                let update = self.tab_by_page_mut(page).map(|tab| {
                    tab.set_url(url.clone());
                    tab.set_loading(false);
                    tab.id()
                });
                if let Some(id) = update {
                    self.chrome_dirty = true;
                    self.pending_events
                        .push(BrowserEvent::PageLoadFinished { tab: id, url });
                }
            }
            EngineEvent::TitleChanged { page, title } => {
                let update = self.tab_by_page_mut(page).map(|tab| {
                    tab.set_title(title.clone());
                    tab.id()
                });
                if let Some(id) = update {
                    self.chrome_dirty = true;
                    self.pending_events
                        .push(BrowserEvent::TitleChanged { tab: id, title });
                }
            }
            EngineEvent::NewWindowRequested { page, url } => {
                if let Some(id) = self.tab_by_page_mut(page).map(|tab| tab.id()) {
                    self.pending_events.push(BrowserEvent::NewWindowRequested {
                        tab: id,
                        url: url.clone(),
                    });
                }
                // ADR-007: native window already denied by the engine;
                // open a Halley tab with the requested URL.
                if let Err(err) = self.open_tab(Some(url)) {
                    halley_common::log_warn!("new-window tab failed: {err}");
                }
            }
        }
    }

    fn apply_chrome_action(&mut self, action: ChromeAction) -> Result<(), CoreError> {
        // Jerry panel actions are handled by `JerryHost` in the application
        // loop (not tab commands). Surface them as events.
        if is_jerry_action(&action) {
            self.pending_events.push(BrowserEvent::JerryAction(action));
            return Ok(());
        }
        let command = match action {
            ChromeAction::Navigate(input) => BrowserCommand::NavigateInput(input),
            ChromeAction::Back => BrowserCommand::Back,
            ChromeAction::Forward => BrowserCommand::Forward,
            ChromeAction::Reload => BrowserCommand::Reload,
            ChromeAction::Stop => BrowserCommand::Stop,
            ChromeAction::NewTab => BrowserCommand::NewTab { url: None },
            ChromeAction::CloseTab(raw) => match TabId::from_ipc_str(&raw) {
                Some(id) => BrowserCommand::CloseTab(id),
                None => {
                    halley_common::log_debug!("ignoring close for bad tab id {raw:?}");
                    return Ok(());
                }
            },
            ChromeAction::ActivateTab(raw) => match TabId::from_ipc_str(&raw) {
                Some(id) => BrowserCommand::ActivateTab(id),
                None => {
                    halley_common::log_debug!("ignoring activate for bad tab id {raw:?}");
                    return Ok(());
                }
            },
            ChromeAction::NextTab => BrowserCommand::NextTab,
            ChromeAction::PrevTab => BrowserCommand::PrevTab,
            ChromeAction::ReopenClosedTab => BrowserCommand::ReopenClosedTab,
            ChromeAction::DuplicateTab(raw) => match TabId::from_ipc_str(&raw) {
                Some(id) => BrowserCommand::DuplicateTab(id),
                None => return Ok(()),
            },
            ChromeAction::FocusAddressBar => BrowserCommand::FocusAddressBar,
            ChromeAction::CloseOtherTabs(raw) => match TabId::from_ipc_str(&raw) {
                Some(id) => BrowserCommand::CloseOtherTabs(id),
                None => return Ok(()),
            },
            ChromeAction::CloseTabsToRight(raw) => match TabId::from_ipc_str(&raw) {
                Some(id) => BrowserCommand::CloseTabsToRight(id),
                None => return Ok(()),
            },
            ChromeAction::MoveTab { tab_id, to_index } => match TabId::from_ipc_str(&tab_id) {
                Some(id) => BrowserCommand::MoveTab {
                    tab_id: id,
                    to_index,
                },
                None => return Ok(()),
            },
            // Jerry actions are filtered above; defensive no-op.
            other => {
                halley_common::log_debug!("unhandled chrome action {other:?}");
                return Ok(());
            }
        };
        match self.execute(command) {
            Ok(outcome) => {
                if matches!(outcome, CommandOutcome::NotSupported { .. }) {
                    halley_common::log_debug!("command not supported: {outcome:?}");
                }
                Ok(())
            }
            Err(CoreError::Engine(EngineError::InvalidState(_))) => {
                // Race: chrome button clicked after history changed.
                self.chrome_dirty = true;
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    fn tab_by_page_mut(&mut self, page: PageId) -> Option<&mut Tab> {
        self.session.tabs_mut().iter_mut_by_page(page)
    }
}

fn command_short(cmd: &BrowserCommand) -> &'static str {
    match cmd {
        BrowserCommand::NewTab { .. } => "new_tab",
        BrowserCommand::CloseTab(_) => "close_tab",
        BrowserCommand::ActivateTab(_) => "activate_tab",
        BrowserCommand::NextTab => "next_tab",
        BrowserCommand::PrevTab => "prev_tab",
        BrowserCommand::ReopenClosedTab => "reopen_closed",
        BrowserCommand::DuplicateTab(_) => "duplicate_tab",
        BrowserCommand::CloseOtherTabs(_) => "close_others",
        BrowserCommand::CloseTabsToRight(_) => "close_to_right",
        BrowserCommand::MoveTab { .. } => "move_tab",
        BrowserCommand::NavigateInput(_) => "navigate_input",
        BrowserCommand::NavigateUrl(_) => "navigate_url",
        BrowserCommand::Back => "back",
        BrowserCommand::Forward => "forward",
        BrowserCommand::Reload => "reload",
        BrowserCommand::Stop => "stop",
        BrowserCommand::FocusAddressBar => "focus_address",
    }
}

/// Whether this chrome action belongs to the Jerry panel host.
fn is_jerry_action(action: &ChromeAction) -> bool {
    matches!(
        action,
        ChromeAction::JerryToggle
            | ChromeAction::JerrySend(_)
            | ChromeAction::JerryStop
            | ChromeAction::JerryNewConversation
            | ChromeAction::JerryClear
            | ChromeAction::JerryEnable
            | ChromeAction::JerryDisable
            | ChromeAction::JerrySetProvider(_)
            | ChromeAction::JerrySetModel(_)
            | ChromeAction::JerrySetKey(_)
            | ChromeAction::JerryClearKey
            | ChromeAction::JerryTestProvider
    )
}
