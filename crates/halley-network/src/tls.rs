//! TLS policy: certificate verification stays on.

use halley_common::TlsConfig;

/// Secure-transport policy for future real transports / documentation of
/// what the platform webview is expected to do.
///
/// Halley does **not** implement TLS. This type records the non-negotiable
/// rules so a future transport cannot “forget” them:
///
/// * Default `verify_certificates = true`.
/// * Disabling verification requires explicit dev-mode config and fails
///   [`halley_common::Config::validate`] otherwise.
/// * Hostname mismatches and expired certs must fail closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlsPolicy {
    verify_certificates: bool,
}

impl TlsPolicy {
    /// Secure default (verification on).
    pub fn secure_default() -> Self {
        TlsPolicy {
            verify_certificates: true,
        }
    }

    /// From config (still secure unless config was explicitly mutated).
    pub fn from_config(config: &TlsConfig) -> Self {
        TlsPolicy {
            verify_certificates: config.verify_certificates,
        }
    }

    /// Whether certificate chains and hostnames are verified.
    pub fn verifies_certificates(&self) -> bool {
        self.verify_certificates
    }

    /// Whether this policy is safe to ship in a release build.
    pub fn is_release_safe(&self) -> bool {
        self.verify_certificates
    }
}

impl Default for TlsPolicy {
    fn default() -> Self {
        Self::secure_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halley_common::Config;

    #[test]
    fn default_verifies_certificates() {
        let policy = TlsPolicy::default();
        assert!(policy.verifies_certificates());
        assert!(policy.is_release_safe());
        assert!(TlsPolicy::from_config(&Config::default().network.tls).verifies_certificates());
    }

    #[test]
    fn config_validate_rejects_insecure_tls_without_dev_mode() {
        let mut config = Config::default();
        config.network.tls.verify_certificates = false;
        assert!(config.validate().is_err());
        config.app.dev_mode = true;
        assert!(config.validate().is_ok());
        // Even then, TlsPolicy records it as not release-safe.
        assert!(!TlsPolicy::from_config(&config.network.tls).is_release_safe());
    }
}
