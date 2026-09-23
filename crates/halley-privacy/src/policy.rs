//! Centralized [`PrivacyPolicy`] composition.
//!
//! One owner for cookie / storage / cache / referrer / permission
//! decisions so privacy is not scattered as ad-hoc `if private_mode`
//! checks across the application.

use halley_common::Config;

use crate::cache::CachePolicy;
use crate::cookie::CookiePolicy;
use crate::permission::PermissionPolicy;
use crate::referrer::ReferrerPolicy;

/// Aggregate privacy policy for a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyPolicy {
    /// Cookie allow rules (first/third party, private ephemeral).
    pub cookies: CookiePolicy,
    /// Whether private storage must never touch the normal profile tree.
    pub private_isolated: bool,
    /// HTTP/page cache rules.
    pub cache: CachePolicy,
    /// Default referrer policy for navigations/subresources.
    pub referrer: ReferrerPolicy,
    /// Permission defaults and in-memory origin grants live behind
    /// interior mutability at the call site; here we keep the default set.
    pub block_trackers_requested: bool,
    /// Telemetry remains off unless a future human-approved design exists.
    pub telemetry_allowed: bool,
}

impl PrivacyPolicy {
    /// Privacy-preserving policy derived from application config.
    pub fn from_config(config: &Config) -> Self {
        PrivacyPolicy {
            cookies: CookiePolicy::from_config(config),
            private_isolated: true,
            cache: CachePolicy::from_config(config.storage.cache_enabled),
            referrer: ReferrerPolicy::StrictOriginWhenCrossOrigin,
            block_trackers_requested: config.privacy.block_trackers,
            telemetry_allowed: config.privacy.telemetry_enabled,
        }
    }

    /// Fresh permission policy matching this privacy stance.
    pub fn permissions(&self) -> PermissionPolicy {
        PermissionPolicy::secure_default()
    }
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self::from_config(&Config::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_privacy_preserving() {
        let policy = PrivacyPolicy::default();
        assert!(!policy.cookies.allow_third_party);
        assert!(policy.private_isolated);
        assert_eq!(policy.referrer, ReferrerPolicy::StrictOriginWhenCrossOrigin);
        assert!(policy.block_trackers_requested);
        assert!(!policy.telemetry_allowed);
        assert!(policy.cache.private_ephemeral_cache);
    }

    #[test]
    fn permissions_match_secure_default() {
        let policy = PrivacyPolicy::default();
        assert_eq!(
            policy.permissions().default_decision(),
            crate::permission::PermissionDecision::Ask
        );
    }
}
