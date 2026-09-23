//! HTTP transport for BYOK provider calls.
//!
//! * [`UreqTransport`] — real HTTPS (feature of this crate); used only when
//!   the user triggers a send/test with a configured provider.
//! * [`MockTransport`] — records calls and returns canned bodies (tests;
//!   no network I/O).
//!
//! Endpoint allowlist lives in [`crate::provider::validate_endpoint`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use url::Url;

use crate::error::JerryError;
use crate::provider::validate_endpoint;

/// HTTP method subset we use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// GET (health / models probes).
    Get,
    /// POST (chat completions).
    Post,
}

impl HttpMethod {
    /// Wire token.
    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
        }
    }
}

/// One outbound provider request (headers may include Authorization).
#[derive(Debug, Clone)]
pub struct ProviderRequest {
    /// HTTP method.
    pub method: HttpMethod,
    /// Absolute URL (already allowlisted by the runtime).
    pub url: Url,
    /// Header name/value pairs (values may include secrets — never log).
    pub headers: Vec<(String, String)>,
    /// JSON body for POST.
    pub body: Option<String>,
}

impl ProviderRequest {
    /// POST JSON helper.
    pub fn post_json(url: Url, headers: Vec<(String, String)>, body: String) -> Self {
        Self {
            method: HttpMethod::Post,
            url,
            headers,
            body: Some(body),
        }
    }

    /// GET helper.
    pub fn get(url: Url, headers: Vec<(String, String)>) -> Self {
        Self {
            method: HttpMethod::Get,
            url,
            headers,
            body: None,
        }
    }

    /// Redacted one-line description for logs (no headers, no body).
    pub fn log_line(&self) -> String {
        let mut url = self.url.clone();
        // Reuse network-style query redaction without depending on
        // halley-network (jerry graph: only mcp + opt + common).
        if let Some(pairs) = url.query() {
            let redacted = redact_query(pairs);
            url.set_query(Some(&redacted));
        }
        format!("{} {}", self.method.as_str(), url)
    }
}

fn redact_query(query: &str) -> String {
    query
        .split('&')
        .map(|pair| {
            let Some((k, _)) = pair.split_once('=') else {
                return pair.to_string();
            };
            let lower = k.to_ascii_lowercase();
            let secretish = lower.contains("key")
                || lower.contains("token")
                || lower.contains("secret")
                || lower.contains("password")
                || lower.contains("auth");
            if secretish {
                format!("{k}=REDACTED")
            } else {
                pair.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// Streaming chunk from the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamChunk {
    /// Assistant text delta.
    TextDelta(String),
    /// Terminal stop reason token from the wire (`"stop"`, `"length"`, …).
    Stop(String),
}

/// Abstraction over HTTP so tests never touch the network.
///
/// `Sync` is required so [`crate::JerryRuntime`] can move across worker
/// threads for streaming completions (`Arc<dyn ProviderTransport>` must
/// be `Send`).
pub trait ProviderTransport: Send + Sync {
    /// Single-shot request; returns status + body text.
    fn request(&self, req: &ProviderRequest) -> Result<(u16, String), JerryError>;

    /// Streaming request: invoke `on_chunk` for each text delta until done
    /// or `cancel` is set. Returns (status, full_text_assembled).
    fn request_stream(
        &self,
        req: &ProviderRequest,
        cancel: &AtomicBool,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<(u16, String), JerryError>;
}

// ---------------------------------------------------------------------------
// Mock (tests)
// ---------------------------------------------------------------------------

/// Recorded call for assertions (body included; headers included — tests
/// must not print them).
#[derive(Debug, Clone)]
pub struct RecordedCall {
    /// Method as string.
    pub method: String,
    /// URL string.
    pub url: String,
    /// Header pairs as sent.
    pub headers: Vec<(String, String)>,
    /// Body if any.
    pub body: Option<String>,
}

/// Configurable mock transport (no I/O).
#[derive(Default)]
pub struct MockTransport {
    calls: Mutex<Vec<RecordedCall>>,
    /// Response status to return (default 200 when unset via `new`).
    status: u16,
    /// Response body (OpenAI-style by default).
    body: String,
    /// Optional stream deltas; if non-empty, stream mode yields these.
    stream_deltas: Vec<String>,
    /// If set, every request fails with this transport error.
    fail: Option<String>,
}

impl MockTransport {
    /// 200 + minimal OpenAI-compatible completion body.
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            status: 200,
            body: r#"{"choices":[{"message":{"role":"assistant","content":"Hello from mock"},"finish_reason":"stop"}]}"#
                .into(),
            stream_deltas: Vec::new(),
            fail: None,
        }
    }

    /// Custom body/status.
    pub fn with_response(status: u16, body: impl Into<String>) -> Self {
        let mut t = Self::new();
        t.status = status;
        t.body = body.into();
        t
    }

    /// Deltas emitted in stream mode (also used to assemble full text).
    pub fn with_stream_deltas(deltas: Vec<String>) -> Self {
        let mut t = Self::new();
        t.stream_deltas = deltas;
        t
    }

    /// Force transport failure.
    pub fn failing(message: impl Into<String>) -> Self {
        let mut t = Self::new();
        t.fail = Some(message.into());
        t
    }

    /// Snapshot of recorded calls.
    pub fn calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// Number of calls.
    pub fn call_count(&self) -> usize {
        self.calls.lock().map(|c| c.len()).unwrap_or(0)
    }
}

impl ProviderTransport for MockTransport {
    fn request(&self, req: &ProviderRequest) -> Result<(u16, String), JerryError> {
        if let Some(msg) = &self.fail {
            return Err(JerryError::Transport(msg.clone()));
        }
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(RecordedCall {
                method: req.method.as_str().into(),
                url: req.url.to_string(),
                headers: req.headers.clone(),
                body: req.body.clone(),
            });
        }
        if !(200..300).contains(&self.status) {
            return Ok((self.status, self.body.clone()));
        }
        Ok((self.status, self.body.clone()))
    }

    fn request_stream(
        &self,
        req: &ProviderRequest,
        cancel: &AtomicBool,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<(u16, String), JerryError> {
        if let Some(msg) = &self.fail {
            return Err(JerryError::Transport(msg.clone()));
        }
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(RecordedCall {
                method: req.method.as_str().into(),
                url: req.url.to_string(),
                headers: req.headers.clone(),
                body: req.body.clone(),
            });
        }
        let mut assembled = String::new();
        for delta in &self.stream_deltas {
            if cancel.load(Ordering::SeqCst) {
                return Ok((self.status, assembled));
            }
            on_chunk(delta);
            assembled.push_str(delta);
        }
        if self.stream_deltas.is_empty() {
            // Fallback: pretend single delta from non-stream body parse is
            // the caller's job — just return full body as text.
            on_chunk(&self.body);
            return Ok((self.status, self.body.clone()));
        }
        Ok((self.status, assembled))
    }
}

// ---------------------------------------------------------------------------
// Real HTTP (ureq)
// ---------------------------------------------------------------------------

/// Real transport using `ureq` (TLS via rustls). No redirects to other
/// hosts beyond ureq defaults; body read fully for non-stream, line-wise
/// for SSE.
pub struct UreqTransport {
    timeout_ms: u64,
}

impl UreqTransport {
    /// Transport with request timeout (milliseconds).
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            timeout_ms: timeout_ms.max(1),
        }
    }

    fn agent(&self) -> ureq::Agent {
        ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_millis(
                self.timeout_ms.min(10_000),
            ))
            .timeout_read(std::time::Duration::from_millis(self.timeout_ms))
            .timeout_write(std::time::Duration::from_millis(self.timeout_ms))
            .redirects(0)
            .build()
    }

    // ureq::Error is large by design (Status holds a Response); mapped
    // immediately to JerryError at call sites — allow the clippy lint here.
    #[allow(clippy::result_large_err)]
    fn send(&self, req: &ProviderRequest) -> Result<ureq::Response, ureq::Error> {
        let mut request = match req.method {
            HttpMethod::Get => self.agent().get(req.url.as_str()),
            HttpMethod::Post => self.agent().post(req.url.as_str()),
        };
        for (name, value) in &req.headers {
            request = request.set(name, value);
        }
        if let Some(body) = &req.body {
            request = request.set("Content-Type", "application/json");
            return request.send_string(body);
        }
        request.call()
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new(30_000)
    }
}

fn map_ureq_err(err: ureq::Error) -> JerryError {
    match err {
        ureq::Error::Status(status, response) => {
            let mut detail = String::new();
            if let Ok(text) = response.into_string() {
                detail = text.chars().take(400).collect();
            }
            JerryError::ProviderStatus {
                status,
                detail: crate::privacy::redact_secrets_in_text(&detail),
            }
        }
        // ureq Transport errors can embed header values (e.g. a bad
        // Authorization line). Redact before the string leaves this crate.
        ureq::Error::Transport(t) => {
            JerryError::Transport(crate::privacy::redact_secrets_in_text(&t.to_string()))
        }
    }
}

impl ProviderTransport for UreqTransport {
    fn request(&self, req: &ProviderRequest) -> Result<(u16, String), JerryError> {
        validate_endpoint(req.url.as_str())?;
        let response = self.send(req).map_err(map_ureq_err)?;
        let status = response.status();
        let body = response.into_string().map_err(|e| {
            JerryError::Transport(crate::privacy::redact_secrets_in_text(&e.to_string()))
        })?;
        Ok((status, body))
    }

    fn request_stream(
        &self,
        req: &ProviderRequest,
        cancel: &AtomicBool,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<(u16, String), JerryError> {
        // For simplicity and correctness across providers we perform a
        // non-streaming call when stream=false is not required; if the
        // provider was asked to stream, body is SSE text. We still read
        // it fully then parse deltas with cancel checks between events.
        // Mid-body socket abort on cancel is NOT implemented (documented).
        validate_endpoint(req.url.as_str())?;
        let response = self.send(req).map_err(map_ureq_err)?;
        let status = response.status();
        let body = response.into_string().map_err(|e| {
            JerryError::Transport(crate::privacy::redact_secrets_in_text(&e.to_string()))
        })?;

        let mut assembled = String::new();
        if req
            .body
            .as_deref()
            .and_then(|b| serde_json::from_str::<serde_json::Value>(b).ok())
            .and_then(|v| v.get("stream").and_then(|s| s.as_bool()))
            == Some(true)
        {
            for delta in parse_sse_text_deltas(&body) {
                if cancel.load(Ordering::SeqCst) {
                    return Ok((status, assembled));
                }
                on_chunk(&delta);
                assembled.push_str(&delta);
            }
            Ok((status, assembled))
        } else {
            on_chunk(&body);
            Ok((status, body))
        }
    }
}

/// Extract assistant text deltas from an SSE body (OpenAI-style `data:`).
pub fn parse_sse_text_deltas(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
            // OpenAI delta: choices[0].delta.content
            if let Some(content) = value
                .pointer("/choices/0/delta/content")
                .and_then(|c| c.as_str())
            {
                out.push(content.to_string());
                continue;
            }
            // Anthropic content_block_delta
            if let Some(text) = value.pointer("/delta/text").and_then(|c| c.as_str()) {
                out.push(text.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::validate_endpoint;

    fn sample_request() -> ProviderRequest {
        ProviderRequest::post_json(
            validate_endpoint("https://api.openai.com/v1").unwrap(),
            vec![("Authorization".into(), "Bearer sk-test".into())],
            r#"{"model":"m","messages":[]}"#.into(),
        )
    }

    #[test]
    fn mock_records_call_with_auth_header() {
        let mock = MockTransport::new();
        let req = sample_request();
        let (status, body) = mock.request(&req).unwrap();
        assert_eq!(status, 200);
        assert!(body.contains("Hello from mock"));
        let calls = mock.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].method, "POST");
        let auth = calls[0]
            .headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("authorization"))
            .unwrap();
        assert_eq!(auth.1, "Bearer sk-test");
    }

    #[test]
    fn mock_stream_yields_deltas_and_honors_cancel() {
        let mock = MockTransport::with_stream_deltas(vec!["Hel".into(), "lo".into()]);
        let req = sample_request();
        let cancel = AtomicBool::new(false);
        let mut got = String::new();
        let mut n = 0;
        let (status, full) = mock
            .request_stream(&req, &cancel, &mut |d| {
                got.push_str(d);
                n += 1;
            })
            .unwrap();
        assert_eq!(status, 200);
        assert_eq!(got, "Hello");
        assert_eq!(full, "Hello");
        assert_eq!(n, 2);
    }

    #[test]
    fn mock_stream_stops_on_cancel() {
        let mock = MockTransport::with_stream_deltas(vec!["A".into(), "B".into(), "C".into()]);
        let req = sample_request();
        let cancel = AtomicBool::new(false);
        let mut count = 0;
        let cancel_ref = &cancel;
        mock.request_stream(&req, cancel_ref, &mut |d| {
            let _ = d;
            count += 1;
            if count == 1 {
                cancel_ref.store(true, Ordering::SeqCst);
            }
        })
        .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn mock_failure_returns_transport_error() {
        let mock = MockTransport::failing("boom");
        let err = mock.request(&sample_request()).unwrap_err();
        assert!(matches!(err, JerryError::Transport(_)));
    }

    #[test]
    fn log_line_redacts_query_but_not_authorization_in_url() {
        let mut req = sample_request();
        req.url = validate_endpoint("https://example.com/v1?api_key=secret").unwrap();
        let line = req.log_line();
        assert!(!line.contains("api_key=secret"));
        assert!(line.contains("api_key=REDACTED") || line.contains("REDACTED"));
    }

    #[test]
    fn parse_sse_extracts_openai_deltas() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"!\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        let deltas = parse_sse_text_deltas(body);
        assert_eq!(deltas, vec!["Hi", "!"]);
    }

    #[test]
    fn parse_sse_extracts_anthropic_deltas() {
        let body = "data: {\"delta\":{\"text\":\"Claude\"}}\n";
        assert_eq!(parse_sse_text_deltas(body), vec!["Claude"]);
    }

    #[test]
    fn ureq_transport_rejects_non_allowlisted_url() {
        let t = UreqTransport::new(1);
        let req = ProviderRequest::get(Url::parse("http://evil.example/").unwrap(), vec![]);
        // validate_endpoint runs before any I/O.
        let err = t.request(&req).unwrap_err();
        assert!(matches!(err, JerryError::EndpointNotAllowed { .. }));
    }
}
