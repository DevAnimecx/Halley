//! Referrer policy: what URL fragment is sent as `Referer` on requests.
//!
//! Defaults follow the modern web platform default
//! (`strict-origin-when-cross-origin`): full URL only for same-origin
//! HTTPS→HTTPS, origin only for cross-origin or downgrades, nothing when
//! the source is stronger than the destination in unsafe ways.

use halley_common::Origin;

/// Browser-level referrer policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReferrerPolicy {
    /// Never send a `Referer`.
    NoReferrer,
    /// Send only the origin.
    Origin,
    /// Send full URL only for same-origin requests; nothing cross-origin.
    SameOrigin,
    /// Send origin for same-origin; nothing on HTTPS→HTTP downgrade.
    StrictOrigin,
    /// Platform default: full URL same-origin HTTPS→HTTPS; origin for
    /// cross-origin or downgrades; nothing for HTTPS→HTTP with path.
    #[default]
    StrictOriginWhenCrossOrigin,
    /// Always send full URL when policy allows (unsafe for HTTP destinations).
    UnsafeUrl,
}

impl ReferrerPolicy {
    /// Parse a standard policy token (case-insensitive); unknown → default.
    pub fn parse(token: &str) -> ReferrerPolicy {
        match token.trim().to_ascii_lowercase().as_str() {
            "no-referrer" => ReferrerPolicy::NoReferrer,
            "origin" => ReferrerPolicy::Origin,
            "same-origin" => ReferrerPolicy::SameOrigin,
            "strict-origin" => ReferrerPolicy::StrictOrigin,
            "unsafe-url" => ReferrerPolicy::UnsafeUrl,
            _ => ReferrerPolicy::StrictOriginWhenCrossOrigin,
        }
    }
}

/// Compute the `Referer` value for a request, or `None` to omit the header.
///
/// * `source` — origin of the document initiating the request.
/// * `destination` — origin the request is going to.
/// * `source_url` / `destination_url` — full URLs when full-URL policies apply.
pub fn referrer_value(
    policy: ReferrerPolicy,
    source: Option<&Origin>,
    destination: Option<&Origin>,
    source_url: Option<&str>,
    _destination_url: Option<&str>,
) -> Option<String> {
    let source = source?;
    let destination = destination?;
    let same_origin = source == destination;
    let downgrade = source.is_secure() && !destination.is_secure();

    match policy {
        ReferrerPolicy::NoReferrer => None,
        ReferrerPolicy::Origin => Some(source.ascii_serialization()),
        ReferrerPolicy::SameOrigin => {
            if same_origin {
                full_or_origin(source, source_url)
            } else {
                None
            }
        }
        ReferrerPolicy::StrictOrigin => {
            if downgrade {
                None
            } else {
                Some(source.ascii_serialization())
            }
        }
        ReferrerPolicy::StrictOriginWhenCrossOrigin => {
            if downgrade {
                // Never leak path/query over plaintext HTTP.
                None
            } else if same_origin {
                full_or_origin(source, source_url)
            } else {
                Some(source.ascii_serialization())
            }
        }
        ReferrerPolicy::UnsafeUrl => {
            if downgrade && !same_origin {
                None
            } else {
                full_or_origin(source, source_url)
            }
        }
    }
}

fn full_or_origin(source: &Origin, source_url: Option<&str>) -> Option<String> {
    source_url
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .map(str::to_string)
        .or_else(|| Some(source.ascii_serialization()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(url: &str) -> Origin {
        Origin::parse(url).unwrap()
    }

    #[test]
    fn same_origin_sends_full_url_under_default_policy() {
        let src = o("https://example.com/page?q=1");
        let dst = o("https://example.com/other");
        let value = referrer_value(
            ReferrerPolicy::default(),
            Some(&src),
            Some(&dst),
            Some("https://example.com/page?q=1"),
            Some("https://example.com/other"),
        );
        assert_eq!(value.as_deref(), Some("https://example.com/page?q=1"));
    }

    #[test]
    fn cross_origin_https_sends_origin_only() {
        let src = o("https://example.com/secret/path");
        let dst = o("https://other.com/");
        let value = referrer_value(
            ReferrerPolicy::StrictOriginWhenCrossOrigin,
            Some(&src),
            Some(&dst),
            Some("https://example.com/secret/path"),
            Some("https://other.com/"),
        );
        assert_eq!(value.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn https_to_http_downgrade_sends_nothing() {
        let src = o("https://example.com/secret/path");
        let dst = o("http://example.com/");
        let value = referrer_value(
            ReferrerPolicy::StrictOriginWhenCrossOrigin,
            Some(&src),
            Some(&dst),
            Some("https://example.com/secret/path"),
            Some("http://example.com/"),
        );
        assert_eq!(value, None);
    }

    #[test]
    fn no_referrer_never_sends() {
        let src = o("https://example.com/");
        let dst = o("https://example.com/");
        assert_eq!(
            referrer_value(
                ReferrerPolicy::NoReferrer,
                Some(&src),
                Some(&dst),
                Some("https://example.com/x"),
                Some("https://example.com/y"),
            ),
            None
        );
    }

    #[test]
    fn parse_tokens() {
        assert_eq!(
            ReferrerPolicy::parse("no-referrer"),
            ReferrerPolicy::NoReferrer
        );
        assert_eq!(ReferrerPolicy::parse("Origin"), ReferrerPolicy::Origin);
        assert_eq!(
            ReferrerPolicy::parse("bogus"),
            ReferrerPolicy::StrictOriginWhenCrossOrigin
        );
    }
}
