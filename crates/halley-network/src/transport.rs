//! Transport abstraction: policy first, sockets later.
//!
//! Default [`NoTransport`] performs **no I/O** (privacy-preserving default
//! and AGENTS.md test rule). [`MockTransport`] is for unit tests.

use std::collections::VecDeque;

use url::Url;

use crate::error::NetworkError;
use crate::request::{HeaderName, HttpMethod};

/// Request handed to a transport (post-policy).
#[derive(Debug, Clone)]
pub struct TransportRequest {
    method: HttpMethod,
    url: Url,
    headers: Vec<(HeaderName, String)>,
    connect_timeout_ms: u64,
    request_timeout_ms: u64,
}

impl TransportRequest {
    /// Build from parts.
    pub fn new(
        method: HttpMethod,
        url: Url,
        headers: Vec<(HeaderName, String)>,
        connect_timeout_ms: u64,
        request_timeout_ms: u64,
    ) -> Self {
        TransportRequest {
            method,
            url,
            headers,
            connect_timeout_ms,
            request_timeout_ms,
        }
    }

    /// Method.
    pub fn method(&self) -> HttpMethod {
        self.method
    }

    /// URL.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Headers.
    pub fn headers(&self) -> &[(HeaderName, String)] {
        &self.headers
    }

    /// Connect timeout ms.
    pub fn connect_timeout_ms(&self) -> u64 {
        self.connect_timeout_ms
    }

    /// Request timeout ms.
    pub fn request_timeout_ms(&self) -> u64 {
        self.request_timeout_ms
    }
}

/// Response from a transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportResponse {
    /// HTTP status code.
    pub status: u16,
    /// Final URL after any transport-level redirects (usually same).
    pub url: Url,
    /// Response headers (name, value).
    pub headers: Vec<(String, String)>,
    /// Optional body for tests (production transports may stream later).
    pub body: Option<Vec<u8>>,
}

/// Send one request.
pub trait Transport {
    /// Execute the request.
    fn send(&mut self, request: &TransportRequest) -> Result<TransportResponse, NetworkError>;
}

/// Default transport: refuses all network I/O.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoTransport;

impl Transport for NoTransport {
    fn send(&mut self, _request: &TransportRequest) -> Result<TransportResponse, NetworkError> {
        Err(NetworkError::NoTransport)
    }
}

/// Scripted transport for tests — **never** opens sockets.
#[derive(Debug, Default)]
pub struct MockTransport {
    /// Queued responses (or errors) consumed in order.
    queue: VecDeque<Result<TransportResponse, NetworkError>>,
    /// Requests observed (method + URL string) for assertions.
    pub sent: Vec<(String, String)>,
}

impl MockTransport {
    /// Empty mock.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue a successful response.
    pub fn push_ok(mut self, status: u16, url: &str) -> Self {
        let url = Url::parse(url).expect("mock url");
        self.queue.push_back(Ok(TransportResponse {
            status,
            url,
            headers: Vec::new(),
            body: None,
        }));
        self
    }

    /// Enqueue a redirect response with Location header.
    pub fn push_redirect(mut self, status: u16, from: &str, location: &str) -> Self {
        let url = Url::parse(from).expect("mock url");
        self.queue.push_back(Ok(TransportResponse {
            status,
            url,
            headers: vec![("location".into(), location.into())],
            body: None,
        }));
        self
    }

    /// Enqueue an error.
    pub fn push_err(mut self, err: NetworkError) -> Self {
        self.queue.push_back(Err(err));
        self
    }
}

impl Transport for MockTransport {
    fn send(&mut self, request: &TransportRequest) -> Result<TransportResponse, NetworkError> {
        self.sent.push((
            request.method().as_str().to_string(),
            request.url().to_string(),
        ));
        self.queue
            .pop_front()
            .unwrap_or(Err(NetworkError::ResponseInvalid))
    }
}
