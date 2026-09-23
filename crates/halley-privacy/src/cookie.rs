//! Cookie model, policy, and in-memory store.
//!
//! **Ownership:** the platform WebView still owns cookies for page traffic
//! (ADR-009). This module is Halley's *policy-layer* store and decision
//! logic — used for first/third-party rules, private-mode isolation, and
//! future adapter wiring. It does not replace or fight the engine jar.
//!
//! Parsing uses the maintained [`cookie`] crate (standards-oriented
//! Set-Cookie handling) rather than a partial hand-rolled parser.

use std::time::{SystemTime, UNIX_EPOCH};

use halley_common::{Config, Origin};
use url::Url;

use crate::error::CookieError;
use crate::party::Party;
use crate::site::SiteContext;

/// Same-site restriction (mirrors the standard attribute).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    /// Cookie only sent in same-site requests.
    Strict,
    /// Cookie sent in same-site and top-level cross-site navigations.
    Lax,
    /// Cookie may be sent cross-site (requires Secure in modern browsers).
    None,
}

impl From<SameSite> for cookie::SameSite {
    fn from(value: SameSite) -> Self {
        match value {
            SameSite::Strict => cookie::SameSite::Strict,
            SameSite::Lax => cookie::SameSite::Lax,
            SameSite::None => cookie::SameSite::None,
        }
    }
}

impl From<cookie::SameSite> for SameSite {
    fn from(value: cookie::SameSite) -> Self {
        match value {
            cookie::SameSite::Strict => SameSite::Strict,
            cookie::SameSite::Lax => SameSite::Lax,
            cookie::SameSite::None => SameSite::None,
        }
    }
}

/// A cookie as stored by Halley's policy layer (not the engine jar).
///
/// `Debug` redacts the value so cookie contents never enter logs.
#[derive(Clone, PartialEq, Eq)]
pub struct Cookie {
    pub(crate) name: String,
    pub(crate) value: String,
    /// Host-only domain (exact host) or domain cookie parent (leading `.`).
    pub(crate) domain: String,
    pub(crate) path: String,
    /// Unix millis; `None` = session cookie.
    pub(crate) expires_ms: Option<u64>,
    pub(crate) secure: bool,
    pub(crate) http_only: bool,
    pub(crate) same_site: SameSite,
    pub(crate) creation_ms: u64,
}

impl std::fmt::Debug for Cookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cookie")
            .field("name", &self.name)
            .field("value", &"REDACTED")
            .field("domain", &self.domain)
            .field("path", &self.path)
            .field("secure", &self.secure)
            .field("http_only", &self.http_only)
            .field("same_site", &self.same_site)
            .finish()
    }
}

impl Cookie {
    /// Cookie name (not secret).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the cookie is HttpOnly (page JS must not receive the value
    /// through Halley APIs).
    pub fn is_http_only(&self) -> bool {
        self.http_only
    }

    /// Whether the cookie requires a secure context.
    pub fn is_secure(&self) -> bool {
        self.secure
    }

    /// Same-site attribute.
    pub fn same_site(&self) -> SameSite {
        self.same_site
    }

    /// Domain attribute as stored.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Path attribute.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Expiry in unix millis if persistent.
    pub fn expires_ms(&self) -> Option<u64> {
        self.expires_ms
    }

    /// Whether this cookie is expired at `now_ms`.
    pub fn is_expired_at(&self, now_ms: u64) -> bool {
        self.expires_ms.is_some_and(|exp| exp <= now_ms)
    }

    /// Serialize as `name=value` for a Cookie request header.
    ///
    /// Callers must only use this on the network path, never in logs or
    /// page-facing APIs for HttpOnly cookies.
    pub fn to_header_pair(&self) -> (&str, &str) {
        (&self.name, &self.value)
    }
}

/// Cookie allow/deny policy (centralized owner: `halley-privacy`).
///
/// Never put real cookie values in `Debug` output used by logs — values
/// are redacted in [`Cookie`]'s `Debug` impl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookiePolicy {
    /// Allow cookies in first-party contexts.
    pub allow_first_party: bool,
    /// Allow cookies in third-party contexts. Default **false** (block).
    pub allow_third_party: bool,
    /// Private sessions use ephemeral jars (never copied to persistent store).
    pub private_mode_ephemeral: bool,
}

impl CookiePolicy {
    /// Privacy-preserving default from `Config::default().cookies`.
    pub fn from_config(config: &Config) -> Self {
        CookiePolicy {
            allow_first_party: true,
            allow_third_party: !config.cookies.block_third_party,
            private_mode_ephemeral: config.cookies.private_mode_ephemeral,
        }
    }

    /// Whether a `Set-Cookie` (or jar write) is allowed in this context.
    pub fn allow_set(&self, party: Party, private_mode: bool) -> bool {
        if private_mode && !self.private_mode_ephemeral {
            // Config asked for non-ephemeral private cookies — still refuse
            // to write them into the *persistent* store; ephemeral only.
            return false;
        }
        match party {
            Party::First => self.allow_first_party,
            Party::Third => self.allow_third_party,
        }
    }

    /// Whether cookies may be *sent* on a request in this context.
    pub fn allow_send(&self, party: Party, private_mode: bool) -> bool {
        if private_mode && !self.private_mode_ephemeral {
            return false;
        }
        match party {
            Party::First => self.allow_first_party,
            Party::Third => self.allow_third_party,
        }
    }
}

impl Default for CookiePolicy {
    fn default() -> Self {
        CookiePolicy {
            allow_first_party: true,
            allow_third_party: false,
            private_mode_ephemeral: true,
        }
    }
}

/// In-memory cookie jar for one profile / private session.
#[derive(Debug, Default)]
pub struct CookieStore {
    cookies: Vec<Cookie>,
    private: bool,
    policy: CookiePolicy,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl CookieStore {
    /// Create an empty store for a normal or private profile.
    pub fn new(private: bool, policy: CookiePolicy) -> Self {
        CookieStore {
            cookies: Vec::new(),
            private,
            policy,
        }
    }

    /// Whether this jar belongs to a private session.
    pub fn is_private(&self) -> bool {
        self.private
    }

    /// Number of non-expired cookies (test/diagnostics).
    pub fn len(&self) -> usize {
        let now = now_ms();
        self.cookies
            .iter()
            .filter(|c| !c.is_expired_at(now))
            .count()
    }

    /// True when the jar has no live cookies.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Parse a `Set-Cookie` header value and store it if policy allows.
    ///
    /// `request_url` and `site` determine secure matching and party
    /// classification. Returns the stored cookie or an error.
    pub fn set_from_header(
        &mut self,
        set_cookie: &str,
        request_url: &str,
        site: &SiteContext,
    ) -> Result<&Cookie, CookieError> {
        let url = Url::parse(request_url).map_err(|_| CookieError::Malformed)?;
        let origin = Origin::from_url(&url).map_err(|_| CookieError::Malformed)?;
        let party = site.classify(Some(&origin));
        if !self.policy.allow_set(party, self.private) {
            return Err(CookieError::BlockedByPolicy);
        }

        let parsed =
            cookie::Cookie::parse(set_cookie.to_string()).map_err(|_| CookieError::Malformed)?;
        if parsed.name().is_empty() {
            return Err(CookieError::MissingName);
        }

        let domain = match parsed.domain() {
            Some(d) => {
                let d = d.trim_start_matches('.').to_ascii_lowercase();
                if d.is_empty() {
                    return Err(CookieError::InvalidDomain);
                }
                d
            }
            None => origin.host().to_string(),
        };
        let path = parsed
            .path()
            .map(str::to_string)
            .unwrap_or_else(|| default_path(url.path()));

        let expires_ms = match parsed.expires() {
            Some(cookie::Expiration::DateTime(dt)) => {
                Some((dt.unix_timestamp().max(0) as u64).saturating_mul(1000))
            }
            _ => None,
        };

        let cookie = Cookie {
            name: parsed.name().to_string(),
            value: parsed.value().to_string(),
            domain,
            path,
            expires_ms,
            secure: parsed.secure().unwrap_or(false),
            http_only: parsed.http_only().unwrap_or(false),
            same_site: parsed
                .same_site()
                .map(SameSite::from)
                .unwrap_or(SameSite::Lax),
            creation_ms: now_ms(),
        };

        // Replace same name/domain/path.
        self.cookies.retain(|c| {
            !(c.name == cookie.name && c.domain == cookie.domain && c.path == cookie.path)
        });
        // Secure cookies only accepted from secure origins.
        if cookie.secure && !origin.is_secure() {
            return Err(CookieError::BlockedByPolicy);
        }
        if let Some(exp) = cookie.expires_ms {
            if exp <= now_ms() {
                // Expired Set-Cookie deletes / is ignored.
                return Err(CookieError::Expired);
            }
        }
        self.cookies.push(cookie);
        Ok(self.cookies.last().expect("just pushed"))
    }

    /// Cookies eligible to send to `url` under policy + security rules.
    ///
    /// HttpOnly cookies are included here (network path only). Callers must
    /// not surface them to page JS.
    pub fn cookies_for_request(&self, url: &str, site: &SiteContext) -> Vec<(String, String)> {
        let Ok(parsed) = Url::parse(url) else {
            return Vec::new();
        };
        let Ok(origin) = Origin::from_url(&parsed) else {
            return Vec::new();
        };
        let party = site.classify(Some(&origin));
        if !self.policy.allow_send(party, self.private) {
            return Vec::new();
        }
        let now = now_ms();
        let path = parsed.path();
        self.cookies
            .iter()
            .filter(|c| !c.is_expired_at(now))
            .filter(|c| {
                if c.secure && !origin.is_secure() {
                    return false;
                }
                domain_matches(&c.domain, origin.host()) && path_matches(&c.path, path)
            })
            .filter(|c| match c.same_site {
                SameSite::Strict | SameSite::Lax => {
                    // Lax/Strict: only same-site or (Lax) top-level GET —
                    // foundation: same-site only for non-document too.
                    site.classify(Some(&origin)) == Party::First
                        || matches!(c.same_site, SameSite::Lax) && party == Party::First
                }
                SameSite::None => true,
            })
            .map(|c| (c.name.clone(), c.value.clone()))
            .collect()
    }

    /// Remove one cookie by name/domain/path (domain+path must match).
    pub fn remove(&mut self, name: &str, domain: &str, path: &str) -> bool {
        let before = self.cookies.len();
        self.cookies
            .retain(|c| !(c.name == name && c.domain == domain && c.path == path));
        self.cookies.len() != before
    }

    /// Remove every cookie for a site host (site-scoped clear).
    pub fn clear_for_site(&mut self, host: &str) -> usize {
        let before = self.cookies.len();
        let host = host.to_ascii_lowercase();
        self.cookies.retain(|c| !domain_matches(&c.domain, &host));
        before - self.cookies.len()
    }

    /// Drop all cookies (normal or private).
    pub fn clear(&mut self) {
        self.cookies.clear();
    }

    /// Erase expired entries (called on close / periodic).
    pub fn purge_expired(&mut self) {
        let now = now_ms();
        self.cookies.retain(|c| !c.is_expired_at(now));
    }
}

fn default_path(url_path: &str) -> String {
    if !url_path.starts_with('/') {
        return "/".to_string();
    }
    match url_path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(idx) => url_path[..idx].to_string(),
    }
}

fn domain_matches(cookie_domain: &str, host: &str) -> bool {
    let cookie_domain = cookie_domain.trim_start_matches('.').to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    if host == cookie_domain {
        return true;
    }
    host.ends_with(&format!(".{cookie_domain}"))
}

fn path_matches(cookie_path: &str, request_path: &str) -> bool {
    if cookie_path == "/" {
        return true;
    }
    if request_path == cookie_path {
        return true;
    }
    request_path.starts_with(cookie_path)
        && (cookie_path.ends_with('/')
            || request_path.as_bytes().get(cookie_path.len()) == Some(&b'/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(url: &str) -> SiteContext {
        SiteContext::for_url(url, false).unwrap()
    }

    #[test]
    fn parses_and_stores_first_party_cookie() {
        let mut store = CookieStore::new(false, CookiePolicy::default());
        let site = site("https://example.com/");
        let c = store
            .set_from_header(
                "sid=abc123; Path=/; HttpOnly; Secure; SameSite=Lax",
                "https://example.com/",
                &site,
            )
            .unwrap();
        assert_eq!(c.name(), "sid");
        assert!(c.is_http_only());
        assert!(c.is_secure());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn blocks_third_party_when_policy_says_so() {
        let mut store = CookieStore::new(false, CookiePolicy::default());
        let site = site("https://example.com/");
        let err = store
            .set_from_header("t=1", "https://tracker.other.com/set", &site)
            .unwrap_err();
        assert_eq!(err, CookieError::BlockedByPolicy);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn rejects_malformed_set_cookie() {
        let mut store = CookieStore::new(false, CookiePolicy::default());
        let site = site("https://example.com/");
        assert_eq!(
            store.set_from_header("not-a-cookie", "https://example.com/", &site),
            Err(CookieError::Malformed)
        );
    }

    #[test]
    fn secure_cookie_not_sent_over_http() {
        let mut store = CookieStore::new(false, CookiePolicy::default());
        let site = site("https://example.com/");
        store
            .set_from_header("sid=secret; Path=/; Secure", "https://example.com/", &site)
            .unwrap();
        let over_https = store.cookies_for_request("https://example.com/", &site);
        assert_eq!(over_https.len(), 1);
        let over_http = store.cookies_for_request("http://example.com/", &site);
        assert!(over_http.is_empty());
    }

    #[test]
    fn third_party_request_does_not_receive_cookies_when_blocked() {
        let mut store = CookieStore::new(false, CookiePolicy::default());
        let site = site("https://example.com/");
        store
            .set_from_header("sid=abc; Path=/", "https://example.com/", &site)
            .unwrap();
        let cross = store.cookies_for_request("https://tracker.other.com/", &site);
        assert!(cross.is_empty());
    }

    #[test]
    fn clear_for_site_only_touches_matching_host() {
        let open = CookiePolicy {
            allow_third_party: true,
            ..Default::default()
        };
        let mut store = CookieStore::new(false, open);
        let site = site("https://example.com/");
        store
            .set_from_header("a=1; Path=/", "https://example.com/", &site)
            .unwrap();
        store
            .set_from_header("b=2; Path=/", "https://tracker.other.com/", &site)
            .unwrap();
        assert_eq!(store.clear_for_site("example.com"), 1);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn http_only_flag_preserved_and_value_not_in_debug() {
        let mut store = CookieStore::new(false, CookiePolicy::default());
        let site = site("https://example.com/");
        store
            .set_from_header(
                "session=SUPER_SECRET; Path=/; HttpOnly",
                "https://example.com/",
                &site,
            )
            .unwrap();
        let cookie = store.cookies.first().unwrap();
        assert!(cookie.is_http_only());
        let debug = format!("{cookie:?}");
        assert!(!debug.contains("SUPER_SECRET"));
        assert!(debug.contains("REDACTED"));
    }

    #[test]
    fn domain_and_path_matching() {
        let mut store = CookieStore::new(false, CookiePolicy::default());
        let site = site("https://example.com/");
        store
            .set_from_header("a=1; Path=/app", "https://example.com/app/index", &site)
            .unwrap();
        let hit = store.cookies_for_request("https://example.com/app/x", &site);
        assert_eq!(hit.len(), 1);
        let miss = store.cookies_for_request("https://example.com/other", &site);
        assert!(miss.is_empty());
        assert!(domain_matches("example.com", "sub.example.com"));
        assert!(!domain_matches("example.com", "notexample.com"));
    }

    #[test]
    fn config_derived_policy_blocks_third_party() {
        let policy = CookiePolicy::from_config(&Config::default());
        assert!(!policy.allow_third_party);
        assert!(policy.allow_first_party);
        assert!(policy.private_mode_ephemeral);
    }
}
