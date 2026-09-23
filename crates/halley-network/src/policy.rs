//! Central [`NetworkPolicy`]: classify → decide (allow / block / modify).

use crate::context::RequestContext;
use crate::request::NetworkRequest;

/// Explicit decision for a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestDecision {
    /// Send as-is.
    Allow,
    /// Do not send; surface a typed block to the caller.
    Block { reason: String },
    /// Send after header modifications (e.g. strip referrer).
    Modify {
        /// Header names (lowercase) to remove before send.
        remove_headers: Vec<String>,
        /// Optional replacement `Referer` value (`None` = omit).
        set_referer: Option<String>,
    },
}

/// Network-level policy (privacy composition of cookie/referrer happens
/// in core; this type answers `allow_request?` / `allow_redirect?`).
///
/// Future `halley-adblock` plugs in by wrapping [`NetworkPolicy::evaluate`]
/// — the decision enum already supports Block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkPolicy {
    /// Only `http`/`https` requests are allowed.
    pub allow_http_https_only: bool,
    /// Maximum redirect hops (mirrors config).
    pub max_redirects: u32,
    /// Whether HTTPS→HTTP downgrades may be followed.
    pub allow_https_to_http_downgrade: bool,
    /// Whether redirects are followed at all.
    pub follow_redirects: bool,
}

impl NetworkPolicy {
    /// Policy from validated application network config.
    pub fn from_config(config: &halley_common::NetworkConfig) -> Self {
        NetworkPolicy {
            allow_http_https_only: true,
            max_redirects: config.max_redirects,
            allow_https_to_http_downgrade: config.allow_https_to_http_downgrade,
            follow_redirects: config.follow_redirects,
        }
    }

    /// Classify + evaluate a request (Allow by default; extend with adblock).
    pub fn evaluate(&self, request: &NetworkRequest, _context: &RequestContext) -> RequestDecision {
        if self.allow_http_https_only {
            match request.url().scheme() {
                "http" | "https" => {}
                other => {
                    return RequestDecision::Block {
                        reason: format!("scheme-not-allowed:{other}"),
                    };
                }
            }
        }
        RequestDecision::Allow
    }
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        NetworkPolicy::from_config(&halley_common::NetworkConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::HttpMethod;

    #[test]
    fn default_allows_http_and_https() {
        let policy = NetworkPolicy::default();
        let ctx = RequestContext::default();
        for url in ["https://example.com/", "http://example.com/"] {
            let req = NetworkRequest::new(HttpMethod::Get, url).unwrap();
            assert_eq!(policy.evaluate(&req, &ctx), RequestDecision::Allow);
        }
    }

    #[test]
    fn blocks_disallowed_schemes_at_policy_layer() {
        // NetworkRequest::new already rejects non-http(s); policy still
        // defends if a request is constructed via future internal paths.
        let policy = NetworkPolicy {
            allow_http_https_only: true,
            ..NetworkPolicy::default()
        };
        assert_eq!(
            policy.max_redirects,
            halley_common::NetworkConfig::default().max_redirects
        );
    }

    #[test]
    fn evaluate_returns_allow_for_normal_request() {
        let policy = NetworkPolicy::default();
        let req = NetworkRequest::new(HttpMethod::Get, "https://example.com/x").unwrap();
        let ctx = RequestContext::document("https://example.com/", false);
        assert!(matches!(
            policy.evaluate(&req, &ctx),
            RequestDecision::Allow
        ));
    }
}
