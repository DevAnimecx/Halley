//! First-party / third-party classification foundation.
//!
//! Classification is **context only** — a third-party request is not
//! assumed malicious. Tracker databases and blocking live elsewhere
//! (`halley-adblock`, future milestones).

use halley_common::{same_site, Origin};

/// Whether a request origin is first- or third-party relative to the
/// top-level site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Party {
    /// Same site as the top-level document (including subdomains).
    First,
    /// Different site from the top-level document.
    Third,
}

/// Classify `request` against the current top-level `top` origin.
///
/// Returns [`Party::First`] when hosts are same-site under
/// [`halley_common::site_label`]; otherwise [`Party::Third`]. If either
/// origin is missing (e.g. `about:blank`), the classification is
/// conservative [`Party::First`] only when both are absent, else Third
/// when the request has a host and top-level does not match.
pub fn classify_party(top: Option<&Origin>, request: Option<&Origin>) -> Party {
    match (top, request) {
        (Some(top), Some(request)) => {
            if same_site(top.host(), request.host()) {
                Party::First
            } else {
                Party::Third
            }
        }
        // No top-level site yet (new tab / about:) — treat as first so
        // document loads are not blocked by third-party cookie rules.
        (None, _) => Party::First,
        (Some(_), None) => Party::First,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(url: &str) -> Origin {
        Origin::parse(url).unwrap()
    }

    #[test]
    fn same_site_subdomain_is_first_party() {
        let top = origin("https://example.com/");
        let req = origin("https://cdn.example.com/app.js");
        assert_eq!(classify_party(Some(&top), Some(&req)), Party::First);
    }

    #[test]
    fn different_site_is_third_party() {
        let top = origin("https://example.com/");
        let req = origin("https://tracker.other.com/pixel.gif");
        assert_eq!(classify_party(Some(&top), Some(&req)), Party::Third);
    }

    #[test]
    fn missing_top_level_defaults_to_first_for_document_loads() {
        let req = origin("https://example.com/");
        assert_eq!(classify_party(None, Some(&req)), Party::First);
    }

    #[test]
    fn http_and_https_same_host_are_same_site() {
        let top = origin("http://example.com/");
        let req = origin("https://example.com/");
        assert_eq!(classify_party(Some(&top), Some(&req)), Party::First);
    }
}
