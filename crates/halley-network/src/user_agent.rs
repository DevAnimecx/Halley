//! Centralized User-Agent policy (one default; no rotation theater).

/// User-Agent string policy for Halley-originated requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserAgentPolicy {
    value: String,
}

impl UserAgentPolicy {
    /// Wrap a UA string (must be non-empty).
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let value = if value.trim().is_empty() {
            halley_common::NetworkConfig::default().user_agent
        } else {
            value
        };
        UserAgentPolicy { value }
    }

    /// From application network config.
    pub fn from_config(config: &halley_common::NetworkConfig) -> Self {
        Self::new(config.user_agent.clone())
    }

    /// Current UA string.
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl Default for UserAgentPolicy {
    fn default() -> Self {
        Self::from_config(&halley_common::NetworkConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_stable_halley_identity_not_a_foreign_browser() {
        let ua = UserAgentPolicy::default();
        assert!(ua.value().contains("Halley"));
        assert!(!ua.value().is_empty());
        // No random rotation API exists.
        assert_eq!(UserAgentPolicy::default(), ua);
    }

    #[test]
    fn empty_config_falls_back_to_default_ua() {
        let ua = UserAgentPolicy::new("   ");
        assert!(ua.value().contains("Halley"));
    }
}
