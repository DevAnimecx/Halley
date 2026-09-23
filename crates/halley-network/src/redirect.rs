//! Redirect policy and hop tracking.

use url::Url;

use crate::error::NetworkError;
use crate::policy::NetworkPolicy;

/// What to do with a redirect response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirectDecision {
    /// Follow `Location` as a new request.
    Follow(Url),
    /// Stop and treat the response as final.
    Stop,
}

/// Counts hops and enforces limit + downgrade rules.
#[derive(Debug, Clone)]
pub struct RedirectTracker {
    hops: u32,
    max_hops: u32,
    allow_downgrade: bool,
}

impl RedirectTracker {
    /// Create from network policy limits.
    pub fn new(policy: &NetworkPolicy) -> Self {
        RedirectTracker {
            hops: 0,
            max_hops: policy.max_redirects,
            allow_downgrade: policy.allow_https_to_http_downgrade,
        }
    }

    /// Hops observed so far.
    pub fn hops(&self) -> u32 {
        self.hops
    }

    /// Validate and consume one redirect hop from `from` to `to`.
    pub fn follow(&mut self, from: &Url, to: &Url) -> Result<RedirectDecision, NetworkError> {
        if self.hops >= self.max_hops {
            return Err(NetworkError::RedirectLimitExceeded);
        }
        if from.scheme() == "https" && to.scheme() == "http" && !self.allow_downgrade {
            return Err(NetworkError::RequestBlocked {
                reason: "https-to-http-downgrade".into(),
            });
        }
        match to.scheme() {
            "http" | "https" => {}
            _ => return Err(NetworkError::InvalidUrl),
        }
        self.hops += 1;
        Ok(RedirectDecision::Follow(to.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halley_common::NetworkConfig;

    fn policy() -> NetworkPolicy {
        NetworkPolicy::from_config(&NetworkConfig {
            max_redirects: 2,
            allow_https_to_http_downgrade: false,
            ..NetworkConfig::default()
        })
    }

    #[test]
    fn allows_hops_under_limit() {
        let mut t = RedirectTracker::new(&policy());
        let a = Url::parse("https://a.example/1").unwrap();
        let b = Url::parse("https://a.example/2").unwrap();
        assert!(t.follow(&a, &b).is_ok());
        assert_eq!(t.hops(), 1);
        let c = Url::parse("https://a.example/3").unwrap();
        assert!(t.follow(&b, &c).is_ok());
        let d = Url::parse("https://a.example/4").unwrap();
        assert_eq!(t.follow(&c, &d), Err(NetworkError::RedirectLimitExceeded));
    }

    #[test]
    fn blocks_https_to_http_downgrade() {
        let mut t = RedirectTracker::new(&policy());
        let a = Url::parse("https://secure.example/").unwrap();
        let b = Url::parse("http://secure.example/").unwrap();
        assert!(matches!(
            t.follow(&a, &b),
            Err(NetworkError::RequestBlocked { .. })
        ));
    }

    #[test]
    fn rejects_non_http_redirect_targets() {
        let mut t = RedirectTracker::new(&policy());
        let a = Url::parse("https://a.example/").unwrap();
        let b = Url::parse("ftp://a.example/").unwrap();
        assert_eq!(t.follow(&a, &b), Err(NetworkError::InvalidUrl));
    }

    #[test]
    fn allows_downgrade_only_when_configured() {
        let cfg = NetworkConfig {
            max_redirects: 5,
            allow_https_to_http_downgrade: true,
            ..NetworkConfig::default()
        };
        let mut t = RedirectTracker::new(&NetworkPolicy::from_config(&cfg));
        let a = Url::parse("https://a.example/").unwrap();
        let b = Url::parse("http://a.example/").unwrap();
        assert!(t.follow(&a, &b).is_ok());
    }
}
