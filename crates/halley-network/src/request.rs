//! Typed network request (metadata only — never a full body buffer).

use url::Url;

use crate::error::NetworkError;

/// HTTP method for Halley-originated requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Patch,
}

impl HttpMethod {
    /// Uppercase token for the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Head => "HEAD",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
        }
    }
}

/// Case-insensitive header name (ASCII).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HeaderName(String);

impl HeaderName {
    /// Create a header name (lowercased for comparison).
    pub fn new(name: &str) -> Self {
        HeaderName(name.trim().to_ascii_lowercase())
    }

    /// Lowercased name as stored.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A network request Halley (or a future fetch tool) would send.
///
/// Bodies are **metadata only** (`body_len`); full bodies are not stored on
/// the type so they cannot be logged by accident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkRequest {
    method: HttpMethod,
    url: Url,
    headers: Vec<(HeaderName, String)>,
    body_len: Option<usize>,
}

impl NetworkRequest {
    /// Build a request; URL must parse.
    pub fn new(method: HttpMethod, url: &str) -> Result<Self, NetworkError> {
        let url = Url::parse(url).map_err(|_| NetworkError::InvalidUrl)?;
        match url.scheme() {
            "http" | "https" => {}
            _ => return Err(NetworkError::InvalidUrl),
        }
        Ok(NetworkRequest {
            method,
            url,
            headers: Vec::new(),
            body_len: None,
        })
    }

    /// HTTP method.
    pub fn method(&self) -> HttpMethod {
        self.method
    }

    /// Parsed URL.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Headers (name lowercased).
    pub fn headers(&self) -> &[(HeaderName, String)] {
        &self.headers
    }

    /// Append a header (value is never logged by this type).
    pub fn with_header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((HeaderName::new(name), value.into()));
        self
    }

    /// Record body length without storing the body.
    pub fn with_body_len(mut self, len: usize) -> Self {
        self.body_len = Some(len);
        self
    }

    /// Body length metadata if known.
    pub fn body_len(&self) -> Option<usize> {
        self.body_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_schemes() {
        assert!(NetworkRequest::new(HttpMethod::Get, "ftp://x/").is_err());
        assert!(NetworkRequest::new(HttpMethod::Get, "not a url").is_err());
        assert!(NetworkRequest::new(HttpMethod::Get, "about:blank").is_err());
    }

    #[test]
    fn accepts_http_and_https() {
        assert!(NetworkRequest::new(HttpMethod::Get, "http://example.com/").is_ok());
        assert!(NetworkRequest::new(HttpMethod::Post, "https://example.com/").is_ok());
    }

    #[test]
    fn header_names_are_normalized() {
        let req = NetworkRequest::new(HttpMethod::Get, "https://example.com/")
            .unwrap()
            .with_header("X-Api-Key", "value-not-logged-here");
        assert_eq!(req.headers()[0].0.as_str(), "x-api-key");
        // Request Debug includes header values — callers must redact before log.
        let _ = req.body_len();
    }
}
