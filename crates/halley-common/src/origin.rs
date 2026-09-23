//! Strongly typed web origins (`scheme://host:port`).
//!
//! Shared by `halley-privacy` and `halley-network` so neither crate depends
//! on the other. Origins are always parsed — webpage- or user-provided
//! strings are never trusted as already-valid origins.

use std::fmt;

use url::Url;

/// Why a string could not be parsed as a web origin.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OriginError {
    /// Input was not an absolute URL.
    InvalidUrl,
    /// URL had no host component.
    MissingHost,
    /// Scheme is not allowed for origin identity (only `http`/`https`).
    UnsupportedScheme(String),
}

impl fmt::Display for OriginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OriginError::InvalidUrl => write!(f, "invalid URL"),
            OriginError::MissingHost => write!(f, "URL has no host"),
            OriginError::UnsupportedScheme(scheme) => {
                write!(f, "unsupported origin scheme: {scheme}")
            }
        }
    }
}

impl std::error::Error for OriginError {}

/// A parsed `http`/`https` origin: scheme, host, and effective port.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Origin {
    scheme: String,
    host: String,
    port: u16,
}

impl Origin {
    /// Parse an absolute URL into an origin (http/https only).
    pub fn parse(url: &str) -> Result<Origin, OriginError> {
        let parsed = Url::parse(url).map_err(|_| OriginError::InvalidUrl)?;
        Self::from_url(&parsed)
    }

    /// Build an origin from an already-parsed URL.
    pub fn from_url(url: &Url) -> Result<Origin, OriginError> {
        let scheme = url.scheme().to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(OriginError::UnsupportedScheme(scheme));
        }
        let host = url
            .host_str()
            .ok_or(OriginError::MissingHost)?
            .to_ascii_lowercase();
        let port = url.port_or_known_default().unwrap_or(default_port(&scheme));
        Ok(Origin { scheme, host, port })
    }

    /// Scheme (`http` or `https`), lowercase.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// Hostname, lowercase (no port, no brackets stripped beyond URL rules).
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Effective port (explicit or scheme default).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Whether this is an `https` origin (secure context for cookies/refs).
    pub fn is_secure(&self) -> bool {
        self.scheme == "https"
    }

    /// Render as `scheme://host` when the port is the scheme default,
    /// otherwise `scheme://host:port`.
    pub fn ascii_serialization(&self) -> String {
        if self.port == default_port(&self.scheme) {
            format!("{}://{}", self.scheme, self.host)
        } else {
            format!("{}://{}:{}", self.scheme, self.host, self.port)
        }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.ascii_serialization())
    }
}

fn default_port(scheme: &str) -> u16 {
    match scheme {
        "https" => 443,
        _ => 80,
    }
}

/// Approximate registrable site label for same-site checks.
///
/// This is **not** a Public Suffix List implementation: it treats the last
/// two dot-labels as the site for ordinary domains (`example.com` from
/// `a.example.com`) and the full host for single-label / IP hosts. Good
/// enough for first/third-party *classification foundation*; a future
/// milestone may adopt a PSL without changing call sites.
pub fn site_label(host: &str) -> String {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return host;
    }
    // IP literals and localhost are their own site.
    if host.parse::<std::net::IpAddr>().is_ok()
        || host == "localhost"
        || host.ends_with(".localhost")
    {
        return host;
    }
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    if labels.len() <= 2 {
        return host;
    }
    // Keep multi-part public-ish suffixes of length 2 only (co.uk would be
    // wrong here — documented limitation).
    format!("{}.{}", labels[labels.len() - 2], labels[labels.len() - 1])
}

/// Whether two hosts are same-site under [`site_label`].
pub fn same_site(host_a: &str, host_b: &str) -> bool {
    site_label(host_a) == site_label(host_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https_with_default_port() {
        let origin = Origin::parse("https://example.com/path?q=1").unwrap();
        assert_eq!(origin.scheme(), "https");
        assert_eq!(origin.host(), "example.com");
        assert_eq!(origin.port(), 443);
        assert!(origin.is_secure());
        assert_eq!(origin.ascii_serialization(), "https://example.com");
    }

    #[test]
    fn parses_http_with_explicit_port() {
        let origin = Origin::parse("http://example.com:8080/").unwrap();
        assert_eq!(origin.port(), 8080);
        assert!(!origin.is_secure());
        assert_eq!(origin.ascii_serialization(), "http://example.com:8080");
    }

    #[test]
    fn rejects_non_http_schemes_and_bad_input() {
        assert_eq!(
            Origin::parse("about:blank"),
            Err(OriginError::UnsupportedScheme("about".into()))
        );
        assert_eq!(Origin::parse("not a url"), Err(OriginError::InvalidUrl));
        assert_eq!(
            Origin::parse("ftp://example.com/"),
            Err(OriginError::UnsupportedScheme("ftp".into()))
        );
    }

    #[test]
    fn host_is_lowercased() {
        let origin = Origin::parse("https://EXAMPLE.Com/").unwrap();
        assert_eq!(origin.host(), "example.com");
    }

    #[test]
    fn site_label_uses_last_two_labels() {
        assert_eq!(site_label("www.example.com"), "example.com");
        assert_eq!(site_label("analytics.example.com"), "example.com");
        assert_eq!(site_label("tracker.other.com"), "other.com");
        assert_eq!(site_label("localhost"), "localhost");
        assert_eq!(site_label("127.0.0.1"), "127.0.0.1");
    }

    #[test]
    fn same_site_classifies_subdomains() {
        assert!(same_site("a.example.com", "b.example.com"));
        assert!(!same_site("example.com", "other.com"));
    }
}
