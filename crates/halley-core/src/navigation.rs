//! Address-bar input → absolute URL.
//!
//! Rules (single tab, MVP):
//!
//! 1. Trim whitespace; empty input is [`NavigationError::Empty`].
//! 2. Explicit `http:`, `https:`, `about:`, or `file:` URLs parse as-is.
//! 3. Other explicit schemes (`javascript:`, `data:`, …) are rejected —
//!    they never reach the engine from the address bar.
//! 4. Scheme-less input that looks like a host (`example.com`,
//!    `localhost:8080/path`, bare IP) becomes `https://…` (or `http://`
//!    for `localhost` / loopback names, where TLS is usually absent).
//! 5. Anything else is a search query against
//!    [`halley_common::SearchProviderConfig`].

use std::fmt;

use halley_common::SearchProviderConfig;
use url::Url;

/// Why address-bar input could not be converted into a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NavigationError {
    /// Input was empty or whitespace-only.
    Empty,
    /// Input used a scheme other than http/https/about/file.
    UnsupportedScheme(String),
    /// Input could not be parsed and did not qualify as a search either.
    InvalidUrl(String),
}

impl fmt::Display for NavigationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NavigationError::Empty => write!(f, "empty address"),
            NavigationError::UnsupportedScheme(scheme) => {
                write!(f, "unsupported URL scheme: {scheme}")
            }
            NavigationError::InvalidUrl(input) => {
                write!(f, "invalid URL: {input}")
            }
        }
    }
}

impl std::error::Error for NavigationError {}

/// Normalize raw address-bar (or CLI) input into an absolute URL string.
///
/// `search` is only consulted for non-URL input; nothing here performs
/// network I/O.
pub fn normalize_address(
    input: &str,
    search: &SearchProviderConfig,
) -> Result<String, NavigationError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(NavigationError::Empty);
    }

    // Explicit scheme?
    if let Some((scheme, _)) = split_scheme(trimmed) {
        match scheme.to_ascii_lowercase().as_str() {
            "http" | "https" | "about" | "file" => {
                return Url::parse(trimmed)
                    .map(|url| url.to_string())
                    .map_err(|_| NavigationError::InvalidUrl(trimmed.to_string()));
            }
            "localhost" => {
                // `localhost:3000` parses as scheme "localhost" with url
                // crate; rewrite to http before giving up.
                let candidate = format!("http://{trimmed}");
                return Url::parse(&candidate)
                    .map(|url| url.to_string())
                    .map_err(|_| NavigationError::InvalidUrl(trimmed.to_string()));
            }
            other => return Err(NavigationError::UnsupportedScheme(other.to_string())),
        }
    }

    // Scheme-less host/path?
    if looks_like_host(trimmed) {
        let scheme = if is_loopback_host(trimmed) {
            "http"
        } else {
            "https"
        };
        let candidate = format!("{scheme}://{trimmed}");
        if let Ok(url) = Url::parse(&candidate) {
            if url.host_str().is_some() {
                return Ok(url.to_string());
            }
        }
    }

    // Search fallback.
    build_search_url(trimmed, search)
        .ok_or_else(|| NavigationError::InvalidUrl(trimmed.to_string()))
}

/// If `input` starts with `scheme:` where scheme is a valid URL scheme
/// token, return `(scheme, rest)`.
fn split_scheme(input: &str) -> Option<(&str, &str)> {
    let idx = input.find(':')?;
    let scheme = &input[..idx];
    if scheme.is_empty() {
        return None;
    }
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return None;
    }
    // Reject `foo bar:baz` style — scheme must touch the colon with no
    // whitespace (already true by slicing at `:`), but also reject when a
    // space appears before the colon (impossible here) or the "scheme"
    // contains a slash (also impossible).
    Some((scheme, &input[idx + 1..]))
}

/// Heuristic: scheme-less input that should be treated as a host rather
/// than a search query (contains a dot, is localhost, or starts with an
/// IP-like first label).
fn looks_like_host(input: &str) -> bool {
    if input.contains(char::is_whitespace) {
        return false;
    }
    // Path or host with explicit port still counts.
    let authority = input.split('/').next().unwrap_or(input);
    if authority.eq_ignore_ascii_case("localhost") || authority.starts_with("localhost:") {
        return true;
    }
    if authority.starts_with('[') {
        // IPv6 literal
        return authority.contains(']');
    }
    let host = authority.split(':').next().unwrap_or(authority);
    if host.is_empty() {
        return false;
    }
    // Dotted name or IPv4.
    host.contains('.') && !host.starts_with('.') && !host.ends_with('.')
}

fn is_loopback_host(input: &str) -> bool {
    let authority = input.split('/').next().unwrap_or(input);
    let host = authority
        .split(':')
        .next()
        .unwrap_or(authority)
        .trim_start_matches('[')
        .trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host == "::1"
        || host.starts_with("127.")
}

fn build_search_url(query: &str, search: &SearchProviderConfig) -> Option<String> {
    if search.base_url.is_empty() || search.query_param.is_empty() {
        return None;
    }
    let mut url = Url::parse(&search.base_url).ok()?;
    {
        let mut pairs = url.query_pairs_mut();
        // Drop any placeholder query the template carried.
        pairs.clear();
        pairs.append_pair(&search.query_param, query);
    }
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search() -> SearchProviderConfig {
        SearchProviderConfig {
            base_url: "https://duckduckgo.com/".to_string(),
            query_param: "q".to_string(),
        }
    }

    #[test]
    fn empty_input_is_rejected() {
        assert_eq!(
            normalize_address("   ", &search()),
            Err(NavigationError::Empty)
        );
    }

    #[test]
    fn explicit_http_https_are_preserved() {
        assert_eq!(
            normalize_address("https://example.com/a?b=1", &search()).unwrap(),
            "https://example.com/a?b=1"
        );
        assert_eq!(
            normalize_address("http://example.com/", &search()).unwrap(),
            "http://example.com/"
        );
    }

    #[test]
    fn about_blank_is_allowed() {
        assert_eq!(
            normalize_address("about:blank", &search()).unwrap(),
            "about:blank"
        );
    }

    #[test]
    fn dangerous_schemes_from_address_bar_are_rejected() {
        for input in [
            "javascript:alert(1)",
            "JAVASCRIPT:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "vbscript:msgbox(1)",
        ] {
            let err = normalize_address(input, &search()).unwrap_err();
            assert!(
                matches!(err, NavigationError::UnsupportedScheme(_)),
                "expected UnsupportedScheme for {input}, got {err:?}"
            );
        }
    }

    #[test]
    fn bare_domains_become_https() {
        assert_eq!(
            normalize_address("example.com", &search()).unwrap(),
            "https://example.com/"
        );
        assert_eq!(
            normalize_address("example.com/path?q=1", &search()).unwrap(),
            "https://example.com/path?q=1"
        );
    }

    #[test]
    fn localhost_uses_http_and_keeps_port() {
        assert_eq!(
            normalize_address("localhost:8080", &search()).unwrap(),
            "http://localhost:8080/"
        );
        assert_eq!(
            normalize_address("localhost:8080/app", &search()).unwrap(),
            "http://localhost:8080/app"
        );
        assert_eq!(
            normalize_address("127.0.0.1:3000", &search()).unwrap(),
            "http://127.0.0.1:3000/"
        );
    }

    #[test]
    fn prose_becomes_a_search_url() {
        let url = normalize_address("rust ownership rules", &search()).unwrap();
        assert!(url.starts_with("https://duckduckgo.com/?q="));
        assert!(url.contains("rust%20ownership%20rules") || url.contains("rust+ownership"));
    }

    #[test]
    fn single_label_without_dot_is_search_not_host() {
        let url = normalize_address("intranet", &search()).unwrap();
        assert!(url.contains("duckduckgo.com"));
        assert!(!url.contains("https://intranet"));
    }

    #[test]
    fn search_base_url_without_query_param_name_is_invalid_template() {
        let bad = SearchProviderConfig {
            base_url: String::new(),
            query_param: "q".to_string(),
        };
        assert!(matches!(
            normalize_address("hello", &bad),
            Err(NavigationError::InvalidUrl(_))
        ));
    }

    #[test]
    fn file_urls_are_allowed() {
        let url = normalize_address("file:///tmp/x.html", &search()).unwrap();
        assert!(url.starts_with("file:"));
    }
}
