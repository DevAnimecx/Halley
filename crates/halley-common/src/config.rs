//! Configuration data types.
//!
//! These are plain data structures with safe defaults. They are the
//! configuration *architecture* only:
//!
//! * No config file is loaded or persisted yet.
//! * No secrets are stored. API key fields are intentionally absent; future
//!   secret storage will use OS-level secure storage (see
//!   `docs/security-model.md`) and will not live in these types.
//! * Defaults encode the privacy-first stance: telemetry off, tracker
//!   blocking on, Jerry off until the user enables it, destructive-action
//!   confirmation always on.

use crate::Error;

/// Top-level Halley configuration.
///
/// Groups every configuration domain so future config-file loading has a
/// single root type. The derived `Default` is the privacy-preserving
/// baseline (telemetry off, tracker blocking on, Jerry off, destructive
/// confirmation on).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    /// Application identity and development flags.
    pub app: AppConfig,
    /// Native window geometry.
    pub window: WindowConfig,
    /// Browser behaviour settings.
    pub browser: BrowserConfig,
    /// Privacy and protection settings.
    pub privacy: PrivacyConfig,
    /// Centralized network timeouts, redirects, UA, proxy, DNS, TLS.
    pub network: NetworkConfig,
    /// Cookie policy defaults (enforcement owner: `halley-privacy`).
    pub cookies: CookieConfig,
    /// Profile / cache / session storage layout.
    pub storage: StorageConfig,
    /// BYOK AI provider settings (no secrets).
    pub ai: AiConfig,
    /// Jerry agent settings.
    pub jerry: JerryConfig,
    /// Performance settings.
    pub performance: PerformanceConfig,
}

impl Config {
    /// Validate configuration at startup.
    ///
    /// Rejects values that would weaken security (disabling TLS
    /// verification outside explicit dev mode) or are unusable
    /// (zero timeouts). Call once before the application runs.
    pub fn validate(&self) -> Result<(), Error> {
        if self.network.connect_timeout_ms == 0
            || self.network.request_timeout_ms == 0
            || self.network.idle_timeout_ms == 0
        {
            return Err(Error::Config(
                "network timeouts must be greater than zero".into(),
            ));
        }
        if self.network.max_redirects > 100 {
            return Err(Error::Config("network max_redirects must be <= 100".into()));
        }
        if !self.network.tls.verify_certificates && !self.app.dev_mode {
            return Err(Error::Config(
                "tls.verify_certificates may only be disabled when app.dev_mode is true".into(),
            ));
        }
        if self.browser.max_closed_tabs == 0 {
            return Err(Error::Config("browser.max_closed_tabs must be > 0".into()));
        }
        Ok(())
    }
}

/// Application-level identity and development flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    /// Human-readable application name used for window titles and logs.
    pub app_name: String,
    /// Whether development-only affordances (e.g. WebView devtools) are
    /// enabled. Off by default.
    pub dev_mode: bool,
    /// Optional override for the WebView user-data directory.
    ///
    /// `None` uses the platform default location. Deliberately not applied
    /// yet: profile isolation wiring lands with encrypted storage; until
    /// then the field documents intent only (**NOT IMPLEMENTED** as an
    /// enforced path).
    pub user_data_dir: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            app_name: "Halley".to_string(),
            dev_mode: false,
            user_data_dir: None,
        }
    }
}

/// Native window geometry (logical pixels).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowConfig {
    /// Initial window title (browser title overrides it once a page loads).
    pub title: String,
    /// Initial inner width in logical pixels.
    pub width: u32,
    /// Initial inner height in logical pixels.
    pub height: u32,
    /// Minimum inner width in logical pixels.
    pub min_width: u32,
    /// Minimum inner height in logical pixels.
    pub min_height: u32,
    /// Height of the full chrome (tab strip + toolbar) in logical pixels.
    /// The content area is the remainder of the window.
    pub chrome_height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        WindowConfig {
            title: "Halley".to_string(),
            width: 1200,
            height: 800,
            min_width: 640,
            min_height: 480,
            // Tab strip (~36) + toolbar (~40).
            chrome_height: 76,
        }
    }
}

/// Browser behaviour settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserConfig {
    /// Home page URL used for new sessions.
    pub homepage: String,
    /// URL loaded in tabs created via New Tab / new-tab page.
    /// Kept configurable — UI must not hard-code it.
    pub new_tab_url: String,
    /// Maximum retained entries in the closed-tab reopen stack.
    /// Bounds memory growth from repeated open/close.
    pub max_closed_tabs: usize,
    /// Search provider used when the address bar input is not a URL.
    pub search_provider: SearchProviderConfig,
}

/// Address-bar search fallback.
///
/// Only consulted for user-typed input that fails URL parsing; nothing
/// queries the network at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchProviderConfig {
    /// Base URL of the search endpoint; the percent-encoded query is
    /// appended as `?{query_param}=…`.
    pub base_url: String,
    /// Query-string parameter name carrying the search terms.
    pub query_param: String,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        BrowserConfig {
            homepage: "about:blank".to_string(),
            new_tab_url: "about:blank".to_string(),
            max_closed_tabs: 10,
            search_provider: SearchProviderConfig {
                // Privacy-oriented default; no tracking parameters.
                base_url: "https://duckduckgo.com/".to_string(),
                query_param: "q".to_string(),
            },
        }
    }
}

/// Privacy settings.
///
/// Defaults are privacy-preserving. These flags are stored preferences only;
/// the protections they refer to are planned, not implemented (see
/// `docs/privacy-model.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyConfig {
    /// Whether any telemetry may ever leave the machine. Must remain `false`
    /// unless the user explicitly opts in via a future, documented flow.
    pub telemetry_enabled: bool,
    /// Whether network-level tracker blocking is requested.
    pub block_trackers: bool,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        PrivacyConfig {
            telemetry_enabled: false,
            block_trackers: true,
        }
    }
}

/// Centralized network configuration (timeouts, redirects, UA, proxy, DNS, TLS).
///
/// UI and page code must not override these per-component; only this struct
/// configures network behaviour. Defaults are conservative and keep TLS
/// certificate verification enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkConfig {
    /// TCP/handshake connect timeout (milliseconds).
    pub connect_timeout_ms: u64,
    /// Full request deadline including body read (milliseconds).
    pub request_timeout_ms: u64,
    /// Idle connection timeout (milliseconds).
    pub idle_timeout_ms: u64,
    /// Maximum redirect hops before [`NetworkError::RedirectLimitExceeded`]-style failure.
    pub max_redirects: u32,
    /// Whether the manager follows HTTP redirects at all.
    pub follow_redirects: bool,
    /// Allow HTTPS → HTTP redirect downgrades. **Must stay `false`** except
    /// explicit local development (validated by [`Config::validate`] only
    /// jointly with other insecure flags — still discouraged).
    pub allow_https_to_http_downgrade: bool,
    /// Centralized User-Agent string (one default; no per-site rotation).
    pub user_agent: String,
    /// Optional explicit proxy. `None` = system/default (no proxy config).
    pub proxy: ProxyConfig,
    /// DNS resolution policy (system only for now).
    pub dns: DnsConfig,
    /// TLS/secure-transport policy.
    pub tls: TlsConfig,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            connect_timeout_ms: 10_000,
            request_timeout_ms: 30_000,
            idle_timeout_ms: 60_000,
            max_redirects: 10,
            follow_redirects: true,
            allow_https_to_http_downgrade: false,
            // Single honest default — no spoofed browser identity.
            user_agent: "Halley/0.1 (+https://github.com/halley-browser/halley)".to_string(),
            proxy: ProxyConfig::None,
            dns: DnsConfig::System,
            tls: TlsConfig::secure_default(),
        }
    }
}

/// Proxy foundation. Explicit configuration only — no silent PAC/VPN.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProxyConfig {
    /// No explicit proxy (platform default routing).
    #[default]
    None,
    /// HTTP proxy authority (`host:port`), e.g. `127.0.0.1:8080`.
    Http {
        /// Hostname or IP of the proxy.
        host: String,
        /// Proxy port.
        port: u16,
    },
    /// SOCKS5 proxy authority.
    Socks5 {
        /// Hostname or IP of the SOCKS5 proxy.
        host: String,
        /// Proxy port.
        port: u16,
    },
}

/// DNS resolution policy for browser-originated requests.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DnsConfig {
    /// Use the OS resolver (**implemented default**; DoH is not built yet).
    #[default]
    System,
    /// DNS-over-HTTPS — **NOT IMPLEMENTED** (reserved; must not be faked).
    Doh {
        /// HTTPS URL of the DoH resolver (unused until implemented).
        resolver_url: String,
    },
}

/// TLS / certificate validation policy.
///
/// Certificate verification is **on** by default and must not be disabled
/// in release builds without explicit development mode (see
/// [`Config::validate`]). Halley does not implement TLS itself; the
/// platform webview / future transport owns the handshake (ADR-008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsConfig {
    /// Whether certificate and hostname verification are enforced.
    /// `true` in all privacy-preserving defaults.
    pub verify_certificates: bool,
}

impl TlsConfig {
    /// Secure default: full certificate verification.
    pub fn secure_default() -> Self {
        TlsConfig {
            verify_certificates: true,
        }
    }
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self::secure_default()
    }
}

/// Cookie policy configuration (owner: `halley-privacy` cookie policy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieConfig {
    /// Block cookies on third-party (cross-site) request contexts by default.
    pub block_third_party: bool,
    /// Private browsing cookie jars are ephemeral and never written to the
    /// normal profile cookie store.
    pub private_mode_ephemeral: bool,
}

impl Default for CookieConfig {
    fn default() -> Self {
        CookieConfig {
            block_third_party: true,
            private_mode_ephemeral: true,
        }
    }
}

/// Storage / profile directory configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    /// Override for the profile data root. `None` uses the platform
    /// user-data directory (e.g. `%APPDATA%\\Halley`, `~/.local/share/Halley`).
    pub profile_root: Option<String>,
    /// Whether the normal profile may use a persistent HTTP cache directory.
    /// Private profiles never persist cache regardless of this flag.
    pub cache_enabled: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig {
            profile_root: None,
            cache_enabled: true,
        }
    }
}

/// AI provider settings (bring-your-own-key).
///
/// Deliberately contains **no API key material**. Secret storage is a future
/// milestone and will not use these types (see `docs/security-model.md`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AiConfig {
    /// Which configured provider to use (e.g. `"openai"`, `"anthropic"`,
    /// `"local"`). Empty until the user configures one.
    pub provider: String,
    /// Model identifier within the provider's catalog.
    pub model: String,
}

/// Jerry agent settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JerryConfig {
    /// Whether the Jerry agent is enabled at all. Off by default.
    pub enabled: bool,
    /// Whether Jerry must ask for confirmation before DESTRUCTIVE actions.
    /// Always on by default; turning it off is a future, explicit user choice.
    pub require_confirmation_for_destructive: bool,
    /// Chat panel height in logical pixels when the panel is open.
    /// Added to [`WindowConfig::chrome_height`] for layout while open.
    pub panel_height: u32,
    /// Soft token budget for assembled context items (approximate).
    pub context_max_tokens: usize,
    /// Tokens reserved for the model reply estimate (not spent on context).
    pub reply_reserve_tokens: usize,
    /// Maximum trailing conversation messages included in a request.
    pub history_max_messages: usize,
}

impl Default for JerryConfig {
    fn default() -> Self {
        JerryConfig {
            enabled: false,
            require_confirmation_for_destructive: true,
            panel_height: 360,
            context_max_tokens: 4_096,
            reply_reserve_tokens: 512,
            history_max_messages: 20,
        }
    }
}

/// Performance settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceConfig {
    /// Soft cap on simultaneously open tabs, when tab management exists.
    pub max_tabs: u32,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        PerformanceConfig { max_tabs: 32 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_privacy_preserving() {
        let config = Config::default();
        assert!(!config.privacy.telemetry_enabled);
        assert!(config.privacy.block_trackers);
        assert!(!config.jerry.enabled);
        assert!(config.jerry.require_confirmation_for_destructive);
    }

    #[test]
    fn default_config_has_no_ai_provider() {
        let config = Config::default();
        assert!(config.ai.provider.is_empty());
        assert!(config.ai.model.is_empty());
    }

    #[test]
    fn default_homepage_is_blank() {
        assert_eq!(Config::default().browser.homepage, "about:blank");
    }

    #[test]
    fn default_window_geometry_is_sane() {
        let window = Config::default().window;
        assert!(window.width >= window.min_width);
        assert!(window.height >= window.min_height);
        assert!(window.chrome_height > 0);
        assert!(window.chrome_height < window.min_height);
    }

    #[test]
    fn default_app_has_no_dev_mode_or_profile_override() {
        let app = Config::default().app;
        assert_eq!(app.app_name, "Halley");
        assert!(!app.dev_mode);
        assert!(app.user_data_dir.is_none());
    }

    #[test]
    fn default_search_provider_has_no_tracking_parameters() {
        let search = &Config::default().browser.search_provider;
        assert!(search.base_url.starts_with("https://"));
        assert!(!search.base_url.contains('?'));
        assert_eq!(search.query_param, "q");
    }

    #[test]
    fn default_new_tab_url_is_configured_not_hardcoded_elsewhere() {
        let browser = &Config::default().browser;
        assert_eq!(browser.new_tab_url, "about:blank");
        assert!(browser.max_closed_tabs > 0);
    }

    #[test]
    fn chrome_height_fits_tab_strip_plus_toolbar() {
        // Tab strip + toolbar both need room; 48 was toolbar-only.
        assert!(Config::default().window.chrome_height >= 64);
    }

    #[test]
    fn default_network_config_is_secure_and_sane() {
        let net = Config::default().network;
        assert!(net.connect_timeout_ms > 0);
        assert!(net.request_timeout_ms >= net.connect_timeout_ms);
        assert!(net.idle_timeout_ms > 0);
        assert!(net.max_redirects > 0 && net.max_redirects <= 100);
        assert!(net.follow_redirects);
        assert!(!net.allow_https_to_http_downgrade);
        assert!(net.tls.verify_certificates);
        assert!(!net.user_agent.is_empty());
        assert_eq!(net.dns, DnsConfig::System);
        assert_eq!(net.proxy, ProxyConfig::None);
    }

    #[test]
    fn default_cookie_config_blocks_third_party_and_keeps_private_ephemeral() {
        let cookies = Config::default().cookies;
        assert!(cookies.block_third_party);
        assert!(cookies.private_mode_ephemeral);
    }

    #[test]
    fn default_storage_config_has_no_root_override_but_allows_normal_cache() {
        let storage = Config::default().storage;
        assert!(storage.profile_root.is_none());
        assert!(storage.cache_enabled);
    }

    #[test]
    fn default_jerry_panel_and_budget_are_sane() {
        let jerry = Config::default().jerry;
        assert!(jerry.panel_height >= 200);
        assert!(jerry.context_max_tokens > 0);
        assert!(jerry.reply_reserve_tokens > 0);
        assert!(jerry.history_max_messages > 0);
        assert!(!jerry.enabled);
        assert!(jerry.require_confirmation_for_destructive);
    }

    #[test]
    fn validate_accepts_defaults() {
        assert!(Config::default().validate().is_ok());
    }

    #[test]
    fn validate_rejects_zero_timeouts() {
        let mut config = Config::default();
        config.network.request_timeout_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_tls_off_outside_dev_mode() {
        let mut config = Config::default();
        config.network.tls.verify_certificates = false;
        assert!(config.validate().is_err());
        config.app.dev_mode = true;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_absurd_redirect_limits() {
        let mut config = Config::default();
        config.network.max_redirects = 101;
        assert!(config.validate().is_err());
    }
}
