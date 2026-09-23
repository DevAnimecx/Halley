//! DNS policy foundation (system resolver only for now).

/// How hostnames are resolved for Halley-originated requests.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DnsPolicy {
    /// OS resolver (**implemented default**).
    #[default]
    System,
    /// DNS-over-HTTPS — **NOT IMPLEMENTED** (must not be faked).
    DohPending {
        /// Configured resolver URL (unused until DoH exists).
        resolver_url: String,
    },
}

impl DnsPolicy {
    /// From application config.
    pub fn from_config(config: &halley_common::NetworkConfig) -> Self {
        match &config.dns {
            halley_common::DnsConfig::System => DnsPolicy::System,
            halley_common::DnsConfig::Doh { resolver_url } => DnsPolicy::DohPending {
                resolver_url: resolver_url.clone(),
            },
        }
    }

    /// Whether this policy can resolve names today.
    pub fn can_resolve_now(&self) -> bool {
        matches!(self, DnsPolicy::System)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_system_and_resolvable() {
        let policy = DnsPolicy::default();
        assert_eq!(policy, DnsPolicy::System);
        assert!(policy.can_resolve_now());
    }

    #[test]
    fn doh_is_not_claimed_as_working() {
        let config = halley_common::NetworkConfig {
            dns: halley_common::DnsConfig::Doh {
                resolver_url: "https://dns.example/dns-query".into(),
            },
            ..Default::default()
        };
        let policy = DnsPolicy::from_config(&config);
        assert!(!policy.can_resolve_now());
    }
}
