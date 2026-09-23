//! Jerry runtime: conversation + context + privacy + provider transport.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use halley_jerry_mcp::ToolRegistry;

use crate::context::{BrowserPageContext, ContextBundle, ContextItem, PageContextProvider};
use crate::conversation::{ConversationStore, Message, MessageRole};
use crate::credentials::CredentialStore;
use crate::error::JerryError;
use crate::privacy::{prepare_message, prepare_system_prompt};
use crate::provider::{join_endpoint, validate_endpoint, ProviderKind, WireDialect};
use crate::response::{StopReason, StructuredResponse};
use crate::transport::{ProviderRequest, ProviderTransport};

/// Default system prompt base (fixed product text).
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are Jerry, Halley's local browser assistant. \
You help the user understand the current page and browsing context. \
You do not control the browser.";

/// Runtime settings (from config; no secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JerrySettings {
    /// Selected provider (empty → not configured).
    pub provider: String,
    /// Selected model id (empty → provider default).
    pub model: String,
    /// Max tokens for assembled context items.
    pub context_max_tokens: usize,
    /// Tokens reserved for the model reply estimate.
    pub reply_reserve_tokens: usize,
    /// Max trailing conversation messages included.
    pub history_max_messages: usize,
    /// Fixed system prompt (privacy suffix is appended automatically).
    pub system_prompt: String,
    /// When true, conversation persists to disk via the store path.
    pub persist_conversations: bool,
}

impl Default for JerrySettings {
    fn default() -> Self {
        Self {
            provider: String::new(),
            model: String::new(),
            context_max_tokens: halley_jerry_opt::DEFAULT_CONTEXT_MAX_TOKENS,
            reply_reserve_tokens: 512,
            history_max_messages: 20,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            persist_conversations: true,
        }
    }
}

/// One user turn handed to the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JerryRequest {
    /// User message text (must be non-empty after trim).
    pub user_text: String,
    /// Browser snapshot for context.
    pub browser: BrowserPageContext,
    /// Optional extra context items (future tools); usually empty.
    pub extra_context: Vec<ContextItem>,
    /// Request streaming deltas via callback.
    pub stream: bool,
}

impl JerryRequest {
    /// Non-streaming request with browser context.
    pub fn new(user_text: impl Into<String>, browser: BrowserPageContext) -> Self {
        Self {
            user_text: user_text.into(),
            browser,
            extra_context: Vec::new(),
            stream: false,
        }
    }

    /// Enable streaming.
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

/// Assembled outbound payload before HTTP (for tests / diagnostics).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledPrompt {
    /// System message content (with privacy suffix).
    pub system: String,
    /// History + new user turn, privacy-prepared.
    pub messages: Vec<Message>,
    /// Context block rendered into the user/system hybrid (see format).
    pub context_block: String,
    /// Context ids kept.
    pub context_ids: Vec<String>,
    /// Estimated prompt tokens (approx).
    pub prompt_tokens_estimate: usize,
}

/// Jerry intelligence runtime.
pub struct JerryRuntime {
    settings: JerrySettings,
    credentials: CredentialStore,
    conversations: ConversationStore,
    transport: Arc<dyn ProviderTransport>,
    tools: ToolRegistry,
    provider: PageContextProvider,
}

impl JerryRuntime {
    /// Runtime with memory-only credentials/conversations (tests, private).
    pub fn memory_only(settings: JerrySettings, transport: Arc<dyn ProviderTransport>) -> Self {
        Self {
            settings,
            credentials: CredentialStore::memory_only(),
            conversations: ConversationStore::new(),
            transport,
            tools: ToolRegistry::with_read_only_tools(),
            provider: PageContextProvider::new(),
        }
    }

    /// Replace transport (tests / host wiring).
    pub fn set_transport(&mut self, transport: Arc<dyn ProviderTransport>) {
        self.transport = transport;
    }

    /// Mutable credential store (host injects profile path store).
    pub fn credentials_mut(&mut self) -> &mut CredentialStore {
        &mut self.credentials
    }

    /// Immutable credentials.
    pub fn credentials(&self) -> &CredentialStore {
        &self.credentials
    }

    /// Mutable conversations (host injects path store).
    pub fn conversations_mut(&mut self) -> &mut ConversationStore {
        &mut self.conversations
    }

    /// Immutable conversations.
    pub fn conversations(&self) -> &ConversationStore {
        &self.conversations
    }

    /// Read-only tool registry advertised to the model (contracts only).
    pub fn tools(&self) -> &ToolRegistry {
        &self.tools
    }

    /// Current settings.
    pub fn settings(&self) -> &JerrySettings {
        &self.settings
    }

    /// Update settings (provider/model/budget).
    pub fn set_settings(&mut self, settings: JerrySettings) {
        self.settings = settings;
    }

    /// Parsed provider kind from settings (if configured).
    pub fn provider_kind(&self) -> Result<ProviderKind, JerryError> {
        if self.settings.provider.trim().is_empty() {
            return Err(JerryError::ProviderNotConfigured);
        }
        ProviderKind::parse(&self.settings.provider)
    }

    /// Effective model id (settings override, else provider default).
    pub fn effective_model(&self) -> Result<String, JerryError> {
        let kind = self.provider_kind()?;
        if !self.settings.model.trim().is_empty() {
            return Ok(self.settings.model.trim().to_string());
        }
        if let Some(cred_model) = self.credentials.get(kind).and_then(|c| c.model.as_ref()) {
            if !cred_model.trim().is_empty() {
                return Ok(cred_model.trim().to_string());
            }
        }
        Ok(kind.spec().default_model.to_string())
    }

    /// Resolve base URL (credential override or default), validated.
    pub fn resolve_base_url(&self) -> Result<url::Url, JerryError> {
        let kind = self.provider_kind()?;
        let default = kind.spec().base_url;
        let custom = self
            .credentials
            .get(kind)
            .and_then(|c| c.base_url.as_deref())
            .unwrap_or(default);
        validate_endpoint(custom)
    }

    /// Build the full prompt assembly without sending (pure).
    pub fn assemble(&self, request: &JerryRequest) -> Result<AssembledPrompt, JerryError> {
        let user_text = request.user_text.trim();
        if user_text.is_empty() {
            return Err(JerryError::InvalidRequest("empty user message".into()));
        }

        let system = prepare_system_prompt(&self.settings.system_prompt);

        let browser_items = self.provider.from_browser(&request.browser);
        let mut items = browser_items;
        items.extend(request.extra_context.iter().cloned());

        let bundle = ContextBundle::budget(
            items,
            self.settings.context_max_tokens,
            self.settings.reply_reserve_tokens,
        );
        let context_block = bundle.render();

        // History from active conversation (or empty).
        let history: Vec<Message> = self
            .conversations
            .active()
            .map(|c| {
                let n = self.settings.history_max_messages;
                c.tail(n).to_vec()
            })
            .unwrap_or_default();

        let mut messages = Vec::with_capacity(history.len() + 2);
        // Context as a system-side note (browser provenance).
        if !context_block.is_empty() {
            messages.push(Message {
                role: MessageRole::System,
                content: format!("<browser_context>\n{context_block}\n</browser_context>"),
                provenance: crate::provenance::Provenance::Browser,
            });
        }
        for m in history {
            if m.role == MessageRole::System && m.content.starts_with("<browser_context>") {
                continue;
            }
            messages.push(prepare_message(&m));
        }
        messages.push(prepare_message(&Message::user(user_text)));

        let prompt_tokens_estimate = halley_jerry_opt::estimate_tokens(&system)
            + messages
                .iter()
                .map(|m| halley_jerry_opt::estimate_tokens(&m.content))
                .sum::<usize>();

        Ok(AssembledPrompt {
            system,
            messages,
            context_block,
            context_ids: bundle.ids(),
            prompt_tokens_estimate,
        })
    }

    /// Validate provider + key + endpoint without sending.
    pub fn check_ready(&self) -> Result<(ProviderKind, String, url::Url), JerryError> {
        let kind = self.provider_kind()?;
        let cred = self
            .credentials
            .get(kind)
            .ok_or(JerryError::MissingApiKey)?;
        if cred.is_empty_key() {
            return Err(JerryError::MissingApiKey);
        }
        let base = self.resolve_base_url()?;
        let model = self.effective_model()?;
        Ok((kind, model, base))
    }

    /// Non-streaming completion: assemble → provider HTTP → structured result.
    ///
    /// Appends user + assistant messages to the active conversation when
    /// `record` is true.
    pub fn complete(
        &mut self,
        request: &JerryRequest,
        cancel: &AtomicBool,
    ) -> Result<StructuredResponse, JerryError> {
        self.send_inner(request, cancel, None, true)
    }

    /// Streaming completion: `on_delta` receives text fragments in order.
    pub fn complete_stream(
        &mut self,
        request: &JerryRequest,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<StructuredResponse, JerryError> {
        self.send_inner(request, cancel, Some(on_delta), true)
    }

    /// Assemble + HTTP without recording history (used by test-connection).
    pub fn send_no_record(
        &mut self,
        request: &JerryRequest,
        cancel: &AtomicBool,
    ) -> Result<StructuredResponse, JerryError> {
        self.send_inner(request, cancel, None, false)
    }

    fn send_inner(
        &mut self,
        request: &JerryRequest,
        cancel: &AtomicBool,
        mut on_delta: Option<&mut dyn FnMut(&str)>,
        record: bool,
    ) -> Result<StructuredResponse, JerryError> {
        let (kind, model, base) = self.check_ready()?;
        let assembled = self.assemble(request)?;
        if cancel.load(Ordering::SeqCst) {
            return Ok(StructuredResponse::cancelled(kind.as_str(), model));
        }

        let key = self
            .credentials
            .get(kind)
            .map(|c| c.api_key.clone())
            .ok_or(JerryError::MissingApiKey)?;

        let spec = kind.spec();
        let wire_request = build_provider_request(
            spec.dialect,
            kind,
            &base,
            &model,
            &key,
            &assembled,
            request.stream,
        )?;

        let stream = request.stream;
        let mut full = String::new();

        let result = if stream {
            let mut delta_fn = |d: &str| {
                full.push_str(d);
                if let Some(cb) = on_delta.as_mut() {
                    cb(d);
                }
            };
            self.transport
                .request_stream(&wire_request, cancel, &mut delta_fn)
        } else {
            self.transport.request(&wire_request)
        };

        let (status, body) = result?;

        if !(200..300).contains(&status) {
            return Err(JerryError::ProviderStatus {
                status,
                detail: body.chars().take(400).collect(),
            });
        }

        let (text, stop) = if stream {
            let stop = extract_stop_reason(spec.dialect, &body).unwrap_or(
                if cancel.load(Ordering::SeqCst) {
                    StopReason::Cancelled
                } else {
                    StopReason::EndTurn
                },
            );
            let text = std::mem::take(&mut full);
            (text, stop)
        } else {
            parse_provider_text(spec.dialect, &body)?
        };

        let completion_tokens = halley_jerry_opt::estimate_tokens(&text);
        let response = StructuredResponse {
            text: text.clone(),
            stop_reason: stop,
            provider: kind.as_str().to_string(),
            model,
            prompt_tokens_estimate: assembled.prompt_tokens_estimate,
            completion_tokens_estimate: completion_tokens,
            context_ids: assembled.context_ids.clone(),
        };

        let should_record = record
            && (!matches!(response.stop_reason, StopReason::Cancelled)
                || !response.text.is_empty());
        if should_record {
            self.record_turn(request.user_text.trim(), &response.text);
        }

        if self.settings.persist_conversations {
            if let Err(err) = self.persist_conversations_to_disk() {
                // Persistence must not fail the turn; surface via log only.
                halley_common::log_warn!(
                    "[{}] conversation persist failed: {err}",
                    crate::SUBSYSTEM
                );
            }
        }

        Ok(response)
    }

    fn record_turn(&mut self, user_text: &str, assistant_text: &str) {
        let id = self
            .conversations
            .active()
            .map(|c| c.id.clone())
            .unwrap_or_else(|| "default".to_string());
        if self.conversations.active().is_none() {
            self.conversations.start_new(id);
        }
        if let Some(conv) = self.conversations.active_mut() {
            conv.push(Message::user(user_text));
            conv.push(Message::assistant(assistant_text));
        }
    }

    /// Best-effort disk save; host may also call store.save_file directly.
    fn persist_conversations_to_disk(&self) -> Result<(), JerryError> {
        // Path is owned by the CredentialStore-style host wiring: if the
        // conversation store was loaded from disk, path is embedded via
        // load_file/save_file by the host. Here we only no-op unless the
        // host replaced the store with a path-aware flow.
        // (JerryHost calls ConversationStore::save_file explicitly.)
        Ok(())
    }

    /// Start a fresh conversation thread.
    pub fn new_conversation(&mut self) -> String {
        let n = self.conversations.conversations.len() + 1;
        let id = format!("conv-{n}");
        self.conversations.start_new(id.clone());
        id
    }

    /// Tiny request for provider connectivity check (no conversation record).
    pub fn test_connection(
        &mut self,
        cancel: &AtomicBool,
    ) -> Result<StructuredResponse, JerryError> {
        let browser = BrowserPageContext::default();
        let request = JerryRequest::new("Reply with the single word: ok", browser);
        self.send_no_record(&request, cancel)
    }
}

fn build_provider_request(
    dialect: WireDialect,
    _kind: ProviderKind,
    base: &url::Url,
    model: &str,
    api_key: &str,
    assembled: &AssembledPrompt,
    stream: bool,
) -> Result<ProviderRequest, JerryError> {
    match dialect {
        WireDialect::OpenAiChat => {
            let url = join_endpoint(base, "chat/completions")?;
            let mut msgs = Vec::new();
            msgs.push(serde_json::json!({"role": "system", "content": assembled.system}));
            for m in &assembled.messages {
                msgs.push(serde_json::json!({
                    "role": match m.role {
                        MessageRole::System => "system",
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                    },
                    "content": m.content,
                }));
            }
            let body = serde_json::json!({
                "model": model,
                "messages": msgs,
                "stream": stream,
            });
            let headers = vec![
                ("Authorization".to_string(), format!("Bearer {api_key}")),
                ("Accept".to_string(), "application/json".to_string()),
            ];
            Ok(ProviderRequest::post_json(url, headers, body.to_string()))
        }
        WireDialect::AnthropicMessages => {
            let url = join_endpoint(base, "v1/messages")?;
            // System separate; only user/assistant in messages.
            let mut msgs = Vec::new();
            for m in &assembled.messages {
                if m.role == MessageRole::System {
                    continue;
                }
                msgs.push(serde_json::json!({
                    "role": match m.role {
                        MessageRole::User => "user",
                        _ => "assistant",
                    },
                    "content": m.content,
                }));
            }
            if msgs.is_empty() {
                return Err(JerryError::InvalidRequest(
                    "no messages for anthropic".into(),
                ));
            }
            let body = serde_json::json!({
                "model": model,
                "max_tokens": 1024,
                "system": assembled.system,
                "messages": msgs,
                "stream": stream,
            });
            let headers = vec![
                ("x-api-key".to_string(), api_key.to_string()),
                ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ("Accept".to_string(), "application/json".to_string()),
            ];
            Ok(ProviderRequest::post_json(url, headers, body.to_string()))
        }
        WireDialect::GeminiGenerate => {
            // Key as header (never query) — avoids URL logging of secrets.
            let url = join_endpoint(
                base,
                &format!("v1beta/models/{model}:streamGenerateContent?alt=sse"),
            )?;
            // Non-stream still uses :generateContent without alt=sse.
            let url = if stream {
                url
            } else {
                join_endpoint(base, &format!("v1beta/models/{model}:generateContent"))?
            };
            let mut contents = Vec::new();
            for m in &assembled.messages {
                let role = match m.role {
                    MessageRole::Assistant => "model",
                    _ => "user",
                };
                contents.push(serde_json::json!({
                    "role": role,
                    "parts": [{"text": m.content}],
                }));
            }
            let body = serde_json::json!({
                "system_instruction": {"parts": [{"text": assembled.system}]},
                "contents": contents,
            });
            let headers = vec![
                ("x-goog-api-key".to_string(), api_key.to_string()),
                ("Accept".to_string(), "application/json".to_string()),
            ];
            Ok(ProviderRequest::post_json(url, headers, body.to_string()))
        }
    }
}

fn parse_provider_text(
    dialect: WireDialect,
    body: &str,
) -> Result<(String, StopReason), JerryError> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|err| JerryError::InvalidResponse(format!("json: {err}")))?;
    match dialect {
        WireDialect::OpenAiChat => {
            let text = value
                .pointer("/choices/0/message/content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            let stop = value
                .pointer("/choices/0/finish_reason")
                .and_then(|c| c.as_str())
                .map(map_finish_reason)
                .unwrap_or(StopReason::EndTurn);
            Ok((text, stop))
        }
        WireDialect::AnthropicMessages => {
            let text = value
                .get("content")
                .and_then(|c| c.as_array())
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                        .collect::<String>()
                })
                .unwrap_or_default();
            let stop = value
                .get("stop_reason")
                .and_then(|c| c.as_str())
                .map(map_finish_reason)
                .unwrap_or(StopReason::EndTurn);
            Ok((text, stop))
        }
        WireDialect::GeminiGenerate => {
            let text = value
                .pointer("/candidates/0/content/parts/0/text")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            Ok((text, StopReason::EndTurn))
        }
    }
}

fn extract_stop_reason(dialect: WireDialect, body: &str) -> Option<StopReason> {
    // Streaming bodies may still embed a final JSON object with finish info.
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    match dialect {
        WireDialect::OpenAiChat => value
            .pointer("/choices/0/finish_reason")
            .and_then(|c| c.as_str())
            .map(map_finish_reason),
        WireDialect::AnthropicMessages => value
            .get("stop_reason")
            .and_then(|c| c.as_str())
            .map(map_finish_reason),
        WireDialect::GeminiGenerate => None,
    }
}

fn map_finish_reason(raw: &str) -> StopReason {
    match raw {
        "length" | "max_tokens" => StopReason::Length,
        "content_filter" | "refusal" => StopReason::Refusal,
        "cancelled" => StopReason::Cancelled,
        _ => StopReason::EndTurn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{HttpMethod, MockTransport};
    use std::sync::atomic::AtomicBool;

    fn runtime_with_mock() -> JerryRuntime {
        let settings = JerrySettings {
            provider: "openai".into(),
            model: "gpt-test".into(),
            ..Default::default()
        };
        let mut rt = JerryRuntime::memory_only(settings, Arc::new(MockTransport::new()));
        rt.credentials_mut()
            .put(
                ProviderKind::OpenAi,
                crate::credentials::ProviderCredential::new("sk-test-key-123456789"),
            )
            .unwrap();
        rt
    }

    #[test]
    fn ready_requires_key() {
        let settings = JerrySettings {
            provider: "openai".into(),
            ..Default::default()
        };
        let rt = JerryRuntime::memory_only(settings, Arc::new(MockTransport::new()));
        assert!(matches!(
            rt.check_ready(),
            Err(JerryError::MissingApiKey) | Err(JerryError::ProviderNotConfigured)
        ));
    }

    #[test]
    fn assemble_frames_context_and_history() {
        let mut rt = runtime_with_mock();
        rt.conversations_mut().ensure_active("default");
        rt.conversations_mut()
            .active_mut()
            .unwrap()
            .push(Message::user("earlier"));
        rt.conversations_mut()
            .active_mut()
            .unwrap()
            .push(Message::assistant("reply"));

        let browser = BrowserPageContext {
            tab_id: Some(1),
            url: "https://example.com/".into(),
            title: "Example".into(),
            private_mode: false,
            open_tab_count: 1,
        };
        let assembled = rt
            .assemble(&JerryRequest::new("what is this?", browser))
            .unwrap();
        assert!(assembled.system.contains("untrusted"));
        assert!(assembled.context_block.contains("[browser]"));
        assert!(assembled.context_ids.contains(&"browser.url".to_string()));
        assert!(assembled
            .messages
            .iter()
            .any(|m| m.content.contains("what is this?")));
        assert!(assembled.prompt_tokens_estimate > 0);
    }

    #[test]
    fn empty_user_message_rejected() {
        let rt = runtime_with_mock();
        let err = rt
            .assemble(&JerryRequest::new("   ", BrowserPageContext::default()))
            .unwrap_err();
        assert!(matches!(err, JerryError::InvalidRequest(_)));
    }

    #[test]
    fn complete_records_conversation_and_sends_auth() {
        // Re-wrap: we need shared access — use a fresh runtime with same mock.
        let settings = JerrySettings {
            provider: "openai".into(),
            model: "gpt-test".into(),
            ..Default::default()
        };
        let shared: Arc<MockTransport> = Arc::new(MockTransport::new());
        let transport: Arc<dyn ProviderTransport> = shared.clone();
        let mut rt2 = JerryRuntime::memory_only(settings, transport);
        rt2.credentials_mut()
            .put(
                ProviderKind::OpenAi,
                crate::credentials::ProviderCredential::new("sk-test-key-123456789"),
            )
            .unwrap();

        let cancel = AtomicBool::new(false);
        let req = JerryRequest::new("hi", BrowserPageContext::default());
        let resp = rt2.complete(&req, &cancel).unwrap();
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert!(resp.text.contains("Hello from mock"));
        assert_eq!(rt2.conversations().active().unwrap().messages.len(), 2);

        let calls = shared.calls();
        assert_eq!(calls.len(), 1);
        let auth = calls[0]
            .headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("authorization"))
            .expect("auth header");
        assert!(auth.1.contains("sk-test-key"));
        // Body should include system with privacy suffix.
        let body = calls[0].body.as_deref().unwrap();
        assert!(body.contains("untrusted") || body.contains("browser_context"));
    }

    #[test]
    fn stream_deltas_reach_callback() {
        let settings = JerrySettings {
            provider: "openai".into(),
            ..Default::default()
        };
        let shared = Arc::new(MockTransport::with_stream_deltas(vec![
            "Hel".into(),
            "lo".into(),
        ]));
        let transport: Arc<dyn ProviderTransport> = shared.clone();
        let mut rt = JerryRuntime::memory_only(settings, transport);
        rt.credentials_mut()
            .put(
                ProviderKind::OpenAi,
                crate::credentials::ProviderCredential::new("sk-test-key-123456789"),
            )
            .unwrap();

        let cancel = AtomicBool::new(false);
        let req = JerryRequest::new("hi", BrowserPageContext::default()).with_stream(true);
        let mut got = String::new();
        let resp = rt
            .complete_stream(&req, &cancel, &mut |d| got.push_str(d))
            .unwrap();
        assert_eq!(got, "Hello");
        assert_eq!(resp.text, "Hello");
    }

    #[test]
    fn cancel_before_send_returns_cancelled() {
        let mut rt = runtime_with_mock();
        let cancel = AtomicBool::new(true);
        let req = JerryRequest::new("hi", BrowserPageContext::default());
        let resp = rt.complete(&req, &cancel).unwrap();
        assert_eq!(resp.stop_reason, StopReason::Cancelled);
        assert_eq!(rt.conversations().active(), None);
    }

    #[test]
    fn anthropic_request_uses_headers_not_query_key() {
        let settings = JerrySettings {
            provider: "anthropic".into(),
            ..Default::default()
        };
        let shared = Arc::new(MockTransport::new());
        let transport: Arc<dyn ProviderTransport> = shared.clone();
        let mut rt = JerryRuntime::memory_only(settings, transport);
        rt.credentials_mut()
            .put(
                ProviderKind::Anthropic,
                crate::credentials::ProviderCredential::new("sk-ant-key-1234567890"),
            )
            .unwrap();

        let cancel = AtomicBool::new(false);
        rt.complete(
            &JerryRequest::new("hi", BrowserPageContext::default()),
            &cancel,
        )
        .unwrap();

        let calls = shared.calls();
        assert!(!calls[0].url.contains("sk-ant"));
        assert!(calls[0]
            .headers
            .iter()
            .any(|(n, v)| n == "x-api-key" && v == "sk-ant-key-1234567890"));
        assert!(calls[0].url.contains("/v1/messages"));
    }

    #[test]
    fn gemini_key_not_in_url() {
        let settings = JerrySettings {
            provider: "gemini".into(),
            ..Default::default()
        };
        let shared = Arc::new(MockTransport::new());
        let transport: Arc<dyn ProviderTransport> = shared.clone();
        let mut rt = JerryRuntime::memory_only(settings, transport);
        rt.credentials_mut()
            .put(
                ProviderKind::Gemini,
                crate::credentials::ProviderCredential::new("AIzaSyTestKey1234567890"),
            )
            .unwrap();
        let cancel = AtomicBool::new(false);
        rt.complete(
            &JerryRequest::new("hi", BrowserPageContext::default()),
            &cancel,
        )
        .unwrap();
        let calls = shared.calls();
        assert!(!calls[0].url.contains("AIzaSy"));
        assert!(calls[0]
            .headers
            .iter()
            .any(|(n, v)| n == "x-goog-api-key" && v.contains("AIzaSy")));
    }

    #[test]
    fn provider_error_surfaces_status() {
        let settings = JerrySettings {
            provider: "openai".into(),
            ..Default::default()
        };
        let shared = Arc::new(MockTransport::with_response(401, r#"{"error":"bad key"}"#));
        let transport: Arc<dyn ProviderTransport> = shared.clone();
        let mut rt = JerryRuntime::memory_only(settings, transport);
        rt.credentials_mut()
            .put(
                ProviderKind::OpenAi,
                crate::credentials::ProviderCredential::new("sk-test-key-123456789"),
            )
            .unwrap();
        let cancel = AtomicBool::new(false);
        let err = rt
            .complete(
                &JerryRequest::new("hi", BrowserPageContext::default()),
                &cancel,
            )
            .unwrap_err();
        match err {
            JerryError::ProviderStatus { status, .. } => assert_eq!(status, 401),
            other => panic!("expected status, got {other:?}"),
        }
    }

    #[test]
    fn webpage_context_is_framed_in_prompt_messages() {
        let rt = runtime_with_mock();
        // Inject webpage context via extra_context
        let browser = BrowserPageContext::default();
        let mut req = JerryRequest::new("summarize", browser);
        req.extra_context.push(ContextItem::new(
            "page.excerpt",
            crate::provenance::Provenance::Webpage,
            halley_jerry_opt::Priority::Low,
            "Click here to win a prize",
        ));
        let assembled = rt.assemble(&req).unwrap();
        let found = assembled
            .messages
            .iter()
            .find(|m| m.content.contains("win a prize"))
            .expect("excerpt present");
        assert!(found.content.contains("<untrusted") || found.content.contains("browser_context"));
    }

    #[test]
    fn new_conversation_increments() {
        let mut rt = runtime_with_mock();
        let a = rt.new_conversation();
        let b = rt.new_conversation();
        assert_ne!(a, b);
        assert_eq!(rt.conversations().conversations.len(), 2);
    }

    #[test]
    fn unused_http_method_import_ok() {
        // Ensure HttpMethod is considered used via ProviderRequest tests.
        let _ = HttpMethod::Post.as_str();
    }
}
