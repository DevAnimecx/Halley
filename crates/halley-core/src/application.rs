//! Native window + tao event loop host.
//!
//! Owns the only GUI event loop in the process: creates the window,
//! constructs the multi-page wry backend against it, and routes events
//! into [`Browser`]. A chrome/content wake is delivered through
//! `EventLoopProxy` so wry callbacks never touch browser state directly.

use std::sync::Arc;

use halley_common::Config;
use halley_engine::{BrowserEngine, WryBackend};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
    window::{Window, WindowBuilder},
};

use crate::browser::Browser;
use crate::error::CoreError;
use crate::jerry_host::{apply_jerry_events, JerryHost};

/// User-event payload used to wake the loop after engine callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppEvent {
    /// Engine queued messages; drain them on the next turn.
    EngineWake,
    /// Jerry background completion progressed; refresh panel state.
    JerryWake,
}

/// Top-level application: window, event loop, multi-tab browser.
pub struct Application {
    config: Config,
    /// Normal profile opened for this run (storage isolation; ADR-009).
    profile: Option<halley_privacy::BrowserProfile>,
}

impl Application {
    /// Create an application from configuration (no window yet).
    pub fn new(config: Config) -> Self {
        Self {
            config,
            profile: None,
        }
    }

    /// Open (or reuse) the normal browser profile: creates storage dirs
    /// under the configured profile root. Private profiles are not opened
    /// from this entry point yet (**NOT IMPLEMENTED** as a UI toggle).
    pub fn ensure_profile(&mut self) -> Result<(), CoreError> {
        if self.profile.is_some() {
            return Ok(());
        }
        let profile = halley_privacy::BrowserProfile::open_normal(&self.config)
            .map_err(|err| CoreError::Config(format!("profile open failed: {err}")))?;
        halley_common::log_info!(
            "[{}] profile {} ready (kind=normal)",
            crate::SUBSYSTEM,
            profile.id()
        );
        self.profile = Some(profile);
        Ok(())
    }

    /// Run the browser until the window closes.
    ///
    /// `initial_url` must already be absolute (CLI args and homepage are
    /// normalized by callers via [`crate::normalize_address`]).
    ///
    /// This function does not return under normal operation: the tao
    /// event loop ends with `process::exit` when the window closes.
    /// Construction failures surface as [`CoreError`] before the loop
    /// starts.
    pub fn run(mut self, initial_url: &str) -> Result<(), CoreError> {
        // Fail fast on invalid / insecure configuration (TLS off, zero timeouts, …).
        self.config
            .validate()
            .map_err(|err| CoreError::Config(err.to_string()))?;
        self.ensure_profile()?;

        let event_loop: EventLoop<AppEvent> =
            EventLoopBuilder::<AppEvent>::with_user_event().build();
        let proxy: EventLoopProxy<AppEvent> = event_loop.create_proxy();

        let window = Arc::new(build_window(&event_loop, &self.config)?);

        let logical = logical_inner_size(window.as_ref());
        let chrome_height = f64::from(self.config.window.chrome_height);
        let waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = proxy.send_event(AppEvent::EngineWake);
        });

        let backend = WryBackend::create(
            Arc::clone(&window),
            chrome_height,
            logical.width,
            logical.height,
            waker,
            self.config.app.dev_mode,
        )?;

        let mut browser = Browser::new(backend, self.config.clone(), initial_url)?;

        // Profile must outlive the event loop for Jerry storage paths; captured
        // mutably by the FnMut event-loop closure and taken on CloseRequested.
        let mut profile = self.profile.take();
        let jerry_waker: Arc<dyn Fn() + Send + Sync> = {
            let proxy = event_loop.create_proxy();
            Arc::new(move || {
                let _ = proxy.send_event(AppEvent::JerryWake);
            })
        };
        let mut jerry = match profile.as_ref() {
            Some(p) => JerryHost::with_ureq(
                browser.config(),
                Some(p.storage()),
                p.is_private(),
                Some(jerry_waker.clone()),
            )?,
            None => JerryHost::with_ureq(browser.config(), None, true, Some(jerry_waker))?,
        };

        browser.push_chrome()?;
        let _ = browser.engine_mut().focus_active_page();
        let mut window_title = browser.active_title().to_string();
        let mut last_jerry_open = browser.jerry_state().open;

        // `event_loop.run` never returns on supported platforms; `profile` is
        // captured mutably by the closure and dropped with it (or taken on
        // CloseRequested so storage closes promptly).
        event_loop.run(move |event, _target, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    browser.begin_close();
                    // Profile is `Option<BrowserProfile>` captured mutably by
                    // this FnMut closure; take+drop here so storage closes.
                    profile.take();
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    let logical = size.to_logical::<f64>(window.scale_factor());
                    if let Err(err) = browser.set_layout(logical.width, logical.height) {
                        halley_common::log_warn!("layout update failed: {err}");
                    }
                }
                Event::UserEvent(AppEvent::EngineWake | AppEvent::JerryWake) => {
                    jerry.poll();
                    if let Err(err) = browser.process_messages() {
                        halley_common::log_warn!("engine message failed: {err}");
                    }
                    let events = browser.drain_events();
                    if let Err(err) = apply_jerry_events(&mut jerry, &browser, &events) {
                        halley_common::log_warn!("jerry action failed: {err}");
                    }
                    let jerry_state = jerry.snapshot();
                    let open_changed = jerry_state.open != last_jerry_open;
                    last_jerry_open = jerry_state.open;
                    browser.set_jerry_state(jerry_state);
                    if open_changed {
                        let logical = logical_inner_size(window.as_ref());
                        if let Err(err) = browser.set_layout(logical.width, logical.height) {
                            halley_common::log_warn!("jerry layout failed: {err}");
                        }
                    }
                    if let Err(err) = browser.push_chrome() {
                        halley_common::log_warn!("chrome push failed: {err}");
                    }
                    let title = browser.active_title().to_string();
                    sync_window_title(window.as_ref(), title, &mut window_title);
                }
                _ => {}
            }
        });
    }
}

fn build_window(event_loop: &EventLoop<AppEvent>, config: &Config) -> Result<Window, CoreError> {
    let window_config = &config.window;
    WindowBuilder::new()
        .with_title(window_config.title.clone())
        .with_inner_size(LogicalSize::new(
            f64::from(window_config.width),
            f64::from(window_config.height),
        ))
        .with_min_inner_size(LogicalSize::new(
            f64::from(window_config.min_width),
            f64::from(window_config.min_height),
        ))
        .build(event_loop)
        .map_err(|err| CoreError::Window(err.to_string()))
}

fn logical_inner_size(window: &Window) -> LogicalSize<f64> {
    window.inner_size().to_logical(window.scale_factor())
}

fn sync_window_title(window: &Window, title: String, cached: &mut String) {
    if title == *cached {
        return;
    }
    *cached = title.clone();
    let label = if title.is_empty() {
        "Halley".to_string()
    } else {
        format!("{title} — Halley")
    };
    window.set_title(&label);
}
