//! Proxy configuration foundation (explicit only).

use halley_common::ProxyConfig;

/// Runtime proxy settings for the manager.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProxySettings {
    /// No explicit proxy.
    #[default]
    None,
    /// HTTP proxy host/port.
    Http { host: String, port: u16 },
    /// SOCKS5 proxy host/port.
    Socks5 { host: String, port: u16 },
}

impl ProxySettings {
    /// From application config.
    pub fn from_config(config: &halley_common::ProxyConfig) -> Self {
        match config {
            ProxyConfig::None => ProxySettings::None,
            ProxyConfig::Http { host, port } => ProxySettings::Http {
                host: host.clone(),
                port: *port,
            },
            ProxyConfig::Socks5 { host, port } => ProxySettings::Socks5 {
                host: host.clone(),
                port: *port,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_no_proxy() {
        assert_eq!(ProxySettings::default(), ProxySettings::None);
        assert_eq!(
            ProxySettings::from_config(&ProxyConfig::None),
            ProxySettings::None
        );
    }

    #[test]
    fn explicit_http_proxy_round_trips() {
        let cfg = ProxyConfig::Http {
            host: "127.0.0.1".into(),
            port: 8080,
        };
        let settings = ProxySettings::from_config(&cfg);
        assert!(matches!(settings, ProxySettings::Http { port: 8080, .. }));
    }
}
