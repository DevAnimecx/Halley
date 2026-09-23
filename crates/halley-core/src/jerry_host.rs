//! Host that wires [`halley_jerry::JerryRuntime`] into chrome Jerry actions.
//!
//! Prompt #5 scope: chat panel state, BYOK credential/conversation paths,
//! background completion with cooperative cancel, and browser page context.
//! **No** autonomous tool execution (tools are contracts only).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use halley_common::Config;
use halley_engine::{BrowserEngine, ChromeAction, JerryChromeMessage, JerryChromeState};
use halley_jerry::{
    BrowserPageContext, ConversationStore, CredentialStore, JerryError, JerryRequest, JerryRuntime,
    JerrySettings, MessageRole, ProviderCredential, ProviderKind, ProviderTransport, StopReason,
    StructuredResponse, UreqTransport,
};
use halley_privacy::{StorageManager, StoragePaths};

use crate::browser::Browser;
use crate::error::CoreError;
use crate::event::BrowserEvent;
use crate::tab::TabId;

/// Filename for conversation memory under the profile category.
pub const CONVERSATIONS_FILE_NAME: &str = "jerry-conversations.json";

/// Shared state observed by the UI while a completion runs on a worker.
struct JobShared {
    /// Accumulated assistant text so far.
    partial: String,
    /// Final result once the worker finishes.
    done: Option<Result<StructuredResponse, String>>,
    /// Context ids from the last successful assemble.
    context_ids: Vec<String>,
}

/// Jerry host: runtime + chrome panel state + optional background job.
///
/// The runtime is moved into the worker for the duration of a streaming
/// completion and restored on [`JerryHost::poll`] (or replaced from disk
/// for path-backed conversation stores).
pub struct JerryHost {
    runtime: Option<JerryRuntime>,
    private: bool,
    panel_open: bool,
    status: String,
    error: Option<String>,
    messages: Vec<JerryChromeMessage>,
    context_note: String,
    cancel: Arc<AtomicBool>,
    busy: Arc<AtomicBool>,
    conversations_path: Option<PathBuf>,
    credentials_path: Option<PathBuf>,
    job: Option<Arc<Mutex<JobShared>>>,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Settings snapshot kept while the runtime is checked out by a worker.
    held_settings: JerrySettings,
    /// Shared slot the worker writes the runtime into when finished.
    returned: Arc<Mutex<Option<JerryRuntime>>>,
}

impl JerryHost {
    /// Host with profile-backed credential/conversation stores.
    ///
    /// Private profiles use memory-only credentials (no disk writes).
    pub fn with_profile(
        config: &Config,
        storage: &StorageManager,
        private: bool,
        transport: Arc<dyn ProviderTransport>,
        waker: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, CoreError> {
        let settings = settings_from_config(config);
        let mut runtime = JerryRuntime::memory_only(settings.clone(), transport);

        let credentials_path = storage.safe_path(
            StoragePaths::Profile,
            halley_jerry::credentials::CREDENTIALS_FILE_NAME,
        )?;
        let conversations_path =
            storage.safe_path(StoragePaths::Profile, CONVERSATIONS_FILE_NAME)?;

        if private {
            *runtime.credentials_mut() = CredentialStore::memory_only();
            return Ok(Self::new_inner(
                runtime, settings, private, None, None, waker,
            ));
        }

        let credentials = CredentialStore::at_path(&credentials_path).map_err(CoreError::Jerry)?;
        *runtime.credentials_mut() = credentials;

        let conversations =
            ConversationStore::load_file(&conversations_path).map_err(CoreError::Jerry)?;
        *runtime.conversations_mut() = conversations;

        Ok(Self::new_inner(
            runtime,
            settings,
            private,
            Some(conversations_path),
            Some(credentials_path),
            waker,
        ))
    }

    /// Host with memory-only stores (tests, private sessions).
    pub fn memory_only(
        config: &Config,
        transport: Arc<dyn ProviderTransport>,
        waker: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Self {
        let settings = settings_from_config(config);
        let runtime = JerryRuntime::memory_only(settings.clone(), transport);
        Self::new_inner(runtime, settings, true, None, None, waker)
    }

    /// Host with real HTTPS transport for BYOK provider calls.
    pub fn with_ureq(
        config: &Config,
        storage: Option<&StorageManager>,
        private: bool,
        waker: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, CoreError> {
        let transport: Arc<dyn ProviderTransport> = Arc::new(UreqTransport::default());
        match storage {
            Some(storage) => Self::with_profile(config, storage, private, transport, waker),
            None => Ok(Self::memory_only(config, transport, waker)),
        }
    }

    fn new_inner(
        runtime: JerryRuntime,
        held_settings: JerrySettings,
        private: bool,
        conversations_path: Option<PathBuf>,
        credentials_path: Option<PathBuf>,
        waker: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Self {
        let status = if held_settings.provider.is_empty() {
            "disabled"
        } else {
            "idle"
        };
        Self {
            runtime: Some(runtime),
            private,
            panel_open: false,
            status: status.to_string(),
            error: None,
            messages: Vec::new(),
            context_note: String::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            busy: Arc::new(AtomicBool::new(false)),
            conversations_path,
            credentials_path,
            job: None,
            waker,
            held_settings,
            returned: Arc::new(Mutex::new(None)),
        }
    }

    /// Whether a completion is in flight (runtime may be checked out).
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// Runtime if currently owned by the host (not checked out).
    fn runtime(&self) -> Option<&JerryRuntime> {
        self.runtime.as_ref()
    }

    fn runtime_mut(&mut self) -> Option<&mut JerryRuntime> {
        self.runtime.as_mut()
    }

    fn require_runtime_mut(&mut self) -> Result<&mut JerryRuntime, CoreError> {
        if self.is_busy() {
            return Err(CoreError::Jerry(JerryError::InvalidRequest(
                "completion in progress".into(),
            )));
        }
        self.runtime.as_mut().ok_or_else(|| {
            CoreError::Jerry(JerryError::InvalidRequest("runtime checked out".into()))
        })
    }

    /// Apply one chrome Jerry action.
    pub fn handle_action<E: BrowserEngine>(
        &mut self,
        action: ChromeAction,
        browser: &Browser<E>,
    ) -> Result<(), CoreError> {
        match action {
            ChromeAction::JerryToggle => {
                self.panel_open = !self.panel_open;
                Ok(())
            }
            ChromeAction::JerryEnable => {
                if self.held_settings.provider.is_empty() {
                    self.held_settings.provider = "openai".to_string();
                }
                let settings = self.held_settings.clone();
                if let Some(rt) = self.runtime_mut() {
                    rt.set_settings(settings);
                }
                if self.status == "disabled" {
                    self.status = "idle".to_string();
                }
                self.error = None;
                Ok(())
            }
            ChromeAction::JerryDisable => {
                self.request_stop();
                if !self.is_busy() {
                    self.status = "disabled".to_string();
                }
                Ok(())
            }
            ChromeAction::JerryStop => {
                self.request_stop();
                Ok(())
            }
            ChromeAction::JerryNewConversation => {
                let rt = self.require_runtime_mut()?;
                rt.new_conversation();
                self.messages.clear();
                self.error = None;
                self.persist_conversations();
                Ok(())
            }
            ChromeAction::JerryClear => {
                let rt = self.require_runtime_mut()?;
                rt.conversations_mut().conversations.clear();
                rt.conversations_mut().active_id = None;
                self.messages.clear();
                self.error = None;
                self.persist_conversations();
                Ok(())
            }
            ChromeAction::JerrySetProvider(raw) => {
                let kind = ProviderKind::parse(&raw)?;
                let mut settings = {
                    let rt = self.require_runtime_mut()?;
                    let mut s = rt.settings().clone();
                    s.provider = kind.as_str().to_string();
                    rt.set_settings(s.clone());
                    rt.credentials_mut().set_selected(kind);
                    s
                };
                settings.provider = kind.as_str().to_string();
                self.held_settings = settings;
                self.error = None;
                if self.status == "disabled" {
                    self.status = "idle".to_string();
                }
                Ok(())
            }
            ChromeAction::JerrySetModel(raw) => {
                let settings = {
                    let rt = self.require_runtime_mut()?;
                    let mut s = rt.settings().clone();
                    s.model = raw.trim().to_string();
                    rt.set_settings(s.clone());
                    s
                };
                self.held_settings = settings;
                self.error = None;
                Ok(())
            }
            ChromeAction::JerrySetKey(key) => {
                let rt = self.require_runtime_mut()?;
                let kind = rt.provider_kind()?;
                rt.credentials_mut()
                    .put(kind, ProviderCredential::new(key))?;
                self.error = None;
                Ok(())
            }
            ChromeAction::JerryClearKey => {
                let rt = self.require_runtime_mut()?;
                let kind = rt.provider_kind()?;
                rt.credentials_mut().remove(kind)?;
                Ok(())
            }
            ChromeAction::JerryTestProvider => self.spawn_test(),
            ChromeAction::JerrySend(text) => self.spawn_send(text, browser),
            _ => Ok(()),
        }
    }

    /// Pull finished job results into panel state (call on each loop wake).
    pub fn poll(&mut self) {
        let Some(job) = self.job.clone() else {
            return;
        };

        let finished = {
            let Ok(mut guard) = job.lock() else {
                return;
            };
            // Refresh streaming bubble from partial text.
            if let Some(last) = self.messages.last_mut() {
                if last.streaming && !guard.partial.is_empty() {
                    last.content = redact_display(&guard.partial);
                }
            }
            if !guard.context_ids.is_empty() {
                let n = guard.context_ids.len();
                self.context_note = format!("{n} context item{}", if n == 1 { "" } else { "s" });
            }
            guard.done.take()
        };
        let Some(result) = finished else {
            return;
        };

        self.job = None;
        self.busy.store(false, Ordering::SeqCst);
        match result {
            Ok(response) => {
                self.status = "idle".to_string();
                self.error = None;
                self.context_note = format!(
                    "{} context item{} · ~{} prompt tok",
                    response.context_ids.len(),
                    if response.context_ids.len() == 1 {
                        ""
                    } else {
                        "s"
                    },
                    response.prompt_tokens_estimate
                );
                self.finish_success(response);
            }
            Err(message) => {
                self.status = "error".to_string();
                self.error = Some(message);
                if let Some(last) = self.messages.last_mut() {
                    if last.streaming && last.content.is_empty() {
                        // keep bubble for honesty about failure
                        last.content = String::from("(failed)");
                        last.streaming = false;
                    } else if last.streaming {
                        last.streaming = false;
                    }
                }
                self.restore_runtime_from_sources();
            }
        }
    }

    /// Chrome-facing panel snapshot.
    pub fn snapshot(&self) -> JerryChromeState {
        let provider = self.held_settings.provider.clone();
        let model = self
            .runtime()
            .and_then(|rt| rt.effective_model().ok())
            .unwrap_or_else(|| self.held_settings.model.clone());
        let has_api_key = ProviderKind::parse(&provider)
            .map(|kind| {
                self.runtime()
                    .map(|rt| rt.credentials().has_key(kind))
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        JerryChromeState {
            open: self.panel_open,
            enabled: !provider.is_empty() && self.status != "disabled",
            provider,
            model,
            has_api_key,
            status: if self.is_busy() {
                "streaming".to_string()
            } else {
                self.status.clone()
            },
            messages: self.messages.clone(),
            context_note: self.context_note.clone(),
            error: self.error.clone(),
        }
    }

    /// Cancel in-flight work (cooperative; between stream deltas).
    pub fn request_stop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    fn spawn_send<E: BrowserEngine>(
        &mut self,
        text: String,
        browser: &Browser<E>,
    ) -> Result<(), CoreError> {
        if self.is_busy() {
            return Ok(());
        }
        let text = text.trim().to_string();
        if text.is_empty() {
            return Ok(());
        }
        {
            let rt = self.runtime().ok_or_else(|| {
                CoreError::Jerry(JerryError::InvalidRequest("runtime out".into()))
            })?;
            rt.check_ready()?;
        }

        let page = page_context(browser, self.private);
        let request = JerryRequest::new(text.clone(), page).with_stream(true);

        self.messages.push(JerryChromeMessage {
            role: "user".into(),
            content: redact_display(&text),
            streaming: false,
        });
        self.messages.push(JerryChromeMessage {
            role: "assistant".into(),
            content: String::new(),
            streaming: true,
        });
        self.status = "connecting".to_string();
        self.error = None;

        self.spawn_job(request)
    }

    fn spawn_test(&mut self) -> Result<(), CoreError> {
        if self.is_busy() {
            return Ok(());
        }
        // Non-streaming probe on a worker so the UI stays responsive.
        let rt = self
            .runtime
            .take()
            .ok_or_else(|| CoreError::Jerry(JerryError::InvalidRequest("runtime out".into())))?;
        rt.check_ready()?;

        self.status = "connecting".to_string();
        self.error = None;
        let cancel = Arc::clone(&self.cancel);
        self.cancel.store(false, Ordering::SeqCst);
        self.busy.store(true, Ordering::SeqCst);

        let job = Arc::new(Mutex::new(JobShared {
            partial: String::new(),
            done: None,
            context_ids: Vec::new(),
        }));
        self.job = Some(Arc::clone(&job));
        let waker = self.waker.clone();
        let conversations_path = self.conversations_path.clone();
        let returned = Arc::clone(&self.returned);

        std::thread::spawn(move || {
            let mut rt = rt;
            let result = match rt.test_connection(&cancel) {
                Ok(response) => {
                    if let (Some(path), true) = (
                        conversations_path.as_ref(),
                        rt.settings().persist_conversations,
                    ) {
                        let _ = rt.conversations().save_file(path);
                    }
                    Ok(response)
                }
                Err(err) => Err(err.to_string()),
            };
            if let Ok(mut j) = job.lock() {
                j.done = Some(result);
            }
            if let Ok(mut slot) = returned.lock() {
                *slot = Some(rt);
            }
            if let Some(w) = waker {
                w();
            }
        });
        Ok(())
    }

    fn spawn_job(&mut self, request: JerryRequest) -> Result<(), CoreError> {
        let rt = self
            .runtime
            .take()
            .ok_or_else(|| CoreError::Jerry(JerryError::InvalidRequest("runtime out".into())))?;

        let cancel = Arc::clone(&self.cancel);
        self.cancel.store(false, Ordering::SeqCst);
        self.busy.store(true, Ordering::SeqCst);

        let job = Arc::new(Mutex::new(JobShared {
            partial: String::new(),
            done: None,
            context_ids: Vec::new(),
        }));
        self.job = Some(Arc::clone(&job));
        let waker = self.waker.clone();
        let conversations_path = self.conversations_path.clone();
        let returned = Arc::clone(&self.returned);

        std::thread::spawn(move || {
            let mut rt = rt;
            let outcome = {
                let mut partial = String::new();
                rt.complete_stream(&request, &cancel, &mut |delta| {
                    partial.push_str(delta);
                    if let Ok(mut j) = job.lock() {
                        j.partial = partial.clone();
                    }
                    if let Some(w) = &waker {
                        w();
                    }
                })
            };
            match outcome {
                Ok(response) => {
                    if let Ok(mut j) = job.lock() {
                        j.context_ids = response.context_ids.clone();
                        j.partial = response.text.clone();
                    }
                    if let (Some(path), true) = (
                        conversations_path.as_ref(),
                        rt.settings().persist_conversations,
                    ) {
                        let _ = rt.conversations().save_file(path);
                    }
                    if let Ok(mut j) = job.lock() {
                        j.done = Some(Ok(response));
                    }
                }
                Err(err) => {
                    if let Ok(mut j) = job.lock() {
                        j.done = Some(Err(err.to_string()));
                    }
                }
            }
            if let Ok(mut slot) = returned.lock() {
                *slot = Some(rt);
            }
            if let Some(w) = waker {
                w();
            }
        });
        Ok(())
    }

    fn finish_success(&mut self, response: StructuredResponse) {
        self.restore_runtime_from_sources();
        if let Some(rt) = self.runtime() {
            if let Some(conv) = rt.conversations().active() {
                if !conv.messages.is_empty() {
                    self.messages = conv
                        .messages
                        .iter()
                        .map(|m| JerryChromeMessage {
                            role: role_str(m.role).to_string(),
                            content: redact_display(&m.content),
                            streaming: false,
                        })
                        .collect();
                    if matches!(response.stop_reason, StopReason::Cancelled) {
                        // Keep partial assistant text already recorded.
                    }
                    return;
                }
            }
        }
        if let Some(last) = self.messages.last_mut() {
            if last.streaming {
                last.content = redact_display(&response.text);
                last.streaming = false;
            }
        }
    }

    fn restore_runtime_from_sources(&mut self) {
        if self.runtime.is_some() {
            return;
        }
        if let Ok(mut slot) = self.returned.lock() {
            if let Some(rt) = slot.take() {
                self.held_settings = rt.settings().clone();
                self.runtime = Some(rt);
                return;
            }
        }
        // Rebuild from disk paths if the worker never returned the runtime.
        let transport: Arc<dyn ProviderTransport> = Arc::new(UreqTransport::default());
        let mut runtime = JerryRuntime::memory_only(self.held_settings.clone(), transport);
        if let Some(path) = self.credentials_path.clone() {
            if let Ok(store) = CredentialStore::at_path(path) {
                *runtime.credentials_mut() = store;
            }
        }
        if let Some(path) = self.conversations_path.clone() {
            if let Ok(store) = ConversationStore::load_file(&path) {
                *runtime.conversations_mut() = store;
            }
        }
        self.runtime = Some(runtime);
    }

    fn persist_conversations(&mut self) {
        if let (Some(path), Some(rt)) = (self.conversations_path.clone(), self.runtime()) {
            if let Err(err) = rt.conversations().save_file(&path) {
                halley_common::log_warn!(
                    "[{}] conversation persist failed: {}",
                    halley_jerry::SUBSYSTEM,
                    err
                );
            }
        }
    }
}

fn settings_from_config(config: &Config) -> JerrySettings {
    JerrySettings {
        provider: config.ai.provider.clone(),
        model: config.ai.model.clone(),
        context_max_tokens: config.jerry.context_max_tokens,
        reply_reserve_tokens: config.jerry.reply_reserve_tokens,
        history_max_messages: config.jerry.history_max_messages,
        ..JerrySettings::default()
    }
}

fn page_context<E: BrowserEngine>(browser: &Browser<E>, private: bool) -> BrowserPageContext {
    BrowserPageContext {
        tab_id: browser.active_tab_id().map(TabId::raw),
        url: browser.active_url().to_string(),
        title: browser.active_title().to_string(),
        private_mode: private,
        open_tab_count: browser.session().tabs().len(),
    }
}

fn role_str(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    }
}

/// Redact obvious secrets before putting text on the chrome UI.
fn redact_display(text: &str) -> String {
    halley_jerry::redact_secrets_in_text(text)
}

/// Drain Jerry actions from browser events and apply them.
pub fn apply_jerry_events<E: BrowserEngine>(
    host: &mut JerryHost,
    browser: &Browser<E>,
    events: &[BrowserEvent],
) -> Result<(), CoreError> {
    for event in events {
        if let BrowserEvent::JerryAction(action) = event {
            host.handle_action(action.clone(), browser)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use halley_engine::MockEngine;
    use halley_jerry::MockTransport;

    fn browser_with_config(config: Config) -> Browser<MockEngine> {
        Browser::new(MockEngine::new(), config, "about:blank").unwrap()
    }

    fn host_with_mock(config: &Config) -> (JerryHost, Arc<MockTransport>) {
        let shared = Arc::new(MockTransport::new());
        let transport: Arc<dyn ProviderTransport> = shared.clone();
        let host = JerryHost::memory_only(config, transport, None);
        (host, shared)
    }

    #[test]
    fn toggle_opens_and_closes_panel() {
        let config = Config::default();
        let browser = browser_with_config(config.clone());
        let (mut host, _) = host_with_mock(&config);
        assert!(!host.snapshot().open);
        host.handle_action(ChromeAction::JerryToggle, &browser)
            .unwrap();
        assert!(host.snapshot().open);
        host.handle_action(ChromeAction::JerryToggle, &browser)
            .unwrap();
        assert!(!host.snapshot().open);
    }

    #[test]
    fn set_provider_and_key_update_snapshot_without_leaking_key() {
        let config = Config::default();
        let browser = browser_with_config(config.clone());
        let (mut host, _) = host_with_mock(&config);
        host.handle_action(ChromeAction::JerrySetProvider("openai".into()), &browser)
            .unwrap();
        host.handle_action(
            ChromeAction::JerrySetKey("sk-test-key-abcdef123456".into()),
            &browser,
        )
        .unwrap();
        let snap = host.snapshot();
        assert_eq!(snap.provider, "openai");
        assert!(snap.has_api_key);
        let debug = format!("{snap:?}");
        assert!(!debug.contains("sk-test-key-abcdef123456"));
    }

    #[test]
    fn send_with_mock_stream_updates_messages() {
        let config = Config::default();
        let browser = browser_with_config(config.clone());
        let shared = Arc::new(MockTransport::with_stream_deltas(vec![
            "Hel".into(),
            "lo".into(),
        ]));
        let transport: Arc<dyn ProviderTransport> = shared.clone();
        let mut host = JerryHost::memory_only(&config, transport, None);
        host.handle_action(ChromeAction::JerrySetProvider("openai".into()), &browser)
            .unwrap();
        host.handle_action(
            ChromeAction::JerrySetKey("sk-test-key-abcdef123456".into()),
            &browser,
        )
        .unwrap();
        host.handle_action(ChromeAction::JerrySend("hi there".into()), &browser)
            .unwrap();
        assert!(host.is_busy());

        for _ in 0..400 {
            host.poll();
            if !host.is_busy() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // Final poll to restore runtime.
        host.poll();
        assert!(!host.is_busy());
        let snap = host.snapshot();
        assert_eq!(snap.status, "idle");
        assert!(snap.messages.iter().any(|m| m.role == "user"));
        let assistant = snap
            .messages
            .iter()
            .find(|m| m.role == "assistant")
            .expect("assistant message");
        assert!(!assistant.streaming);
        assert!(
            assistant.content.contains("Hello") || assistant.content.contains("mock"),
            "got {:?}",
            assistant.content
        );
        assert_eq!(shared.call_count(), 1);
        // Runtime restored for further actions.
        host.handle_action(ChromeAction::JerryToggle, &browser)
            .unwrap();
    }

    #[test]
    fn send_without_key_surfaces_error_before_spawn() {
        let config = Config::default();
        let browser = browser_with_config(config.clone());
        let transport: Arc<dyn ProviderTransport> = Arc::new(MockTransport::new());
        let mut host = JerryHost::memory_only(&config, transport, None);
        host.handle_action(ChromeAction::JerrySetProvider("openai".into()), &browser)
            .unwrap();
        let err = host
            .handle_action(ChromeAction::JerrySend("hi".into()), &browser)
            .unwrap_err();
        assert!(matches!(err, CoreError::Jerry(_)));
        assert!(!host.is_busy());
    }

    #[test]
    fn chrome_events_apply_jerry_actions() {
        let config = Config::default();
        let mut browser = browser_with_config(config.clone());
        let (mut host, _) = host_with_mock(&config);
        browser
            .engine_mut()
            .inject_chrome_action(ChromeAction::JerryToggle);
        browser.process_messages().unwrap();
        let events = browser.drain_events();
        apply_jerry_events(&mut host, &browser, &events).unwrap();
        assert!(host.snapshot().open);
    }

    #[test]
    fn settings_from_config_carry_budgets() {
        let mut config = Config::default();
        config.ai.provider = "groq".into();
        config.ai.model = "llama-x".into();
        config.jerry.context_max_tokens = 2048;
        let settings = settings_from_config(&config);
        assert_eq!(settings.provider, "groq");
        assert_eq!(settings.model, "llama-x");
        assert_eq!(settings.context_max_tokens, 2048);
    }

    #[test]
    fn credential_put_round_trips_via_profile_path() {
        let root =
            std::env::temp_dir().join(format!("halley-jerry-host-{}-prof", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut config = Config::default();
        config.storage.profile_root = Some(root.display().to_string());
        config.validate().unwrap();
        let profile = halley_privacy::BrowserProfile::open_normal(&config).unwrap();
        let transport: Arc<dyn ProviderTransport> = Arc::new(MockTransport::new());
        let mut host =
            JerryHost::with_profile(&config, profile.storage(), false, transport, None).unwrap();
        let browser = browser_with_config(config.clone());
        host.handle_action(ChromeAction::JerrySetProvider("groq".into()), &browser)
            .unwrap();
        host.handle_action(
            ChromeAction::JerrySetKey("gsk_secret_key_value_123456".into()),
            &browser,
        )
        .unwrap();
        drop(host);
        let path = profile
            .storage()
            .safe_path(
                StoragePaths::Profile,
                halley_jerry::credentials::CREDENTIALS_FILE_NAME,
            )
            .unwrap();
        assert!(path.exists());
        let loaded = CredentialStore::at_path(&path).unwrap();
        assert!(loaded.has_key(ProviderKind::Groq));
        profile.close().unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn error_from_jerry_converts() {
        let err: CoreError = JerryError::MissingApiKey.into();
        assert!(err.to_string().contains("API key"));
    }
}
