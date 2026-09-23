//! Top-level site context available to privacy policies.

use halley_common::{site_label, Origin};

use crate::party::{classify_party, Party};

/// The browsing context's top-level site for policy decisions.
///
/// Available to cookie, storage, referrer, permission, and future
/// tracker-blocking / Jerry security policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteContext {
    /// Origin of the top-level document (absent on `about:` pages).
    pub top_level_origin: Option<Origin>,
    /// Approximate site label derived from the top-level host.
    pub top_level_site: String,
    /// Whether this context is a private browsing session.
    pub private_mode: bool,
}

impl SiteContext {
    /// Context with no top-level document yet (startup / about:blank).
    pub fn empty(private_mode: bool) -> Self {
        SiteContext {
            top_level_origin: None,
            top_level_site: String::new(),
            private_mode,
        }
    }

    /// Context for a known top-level URL (must parse as http/https origin).
    pub fn for_url(url: &str, private_mode: bool) -> Result<Self, halley_common::OriginError> {
        let origin = Origin::parse(url)?;
        let top_level_site = site_label(origin.host());
        Ok(SiteContext {
            top_level_origin: Some(origin),
            top_level_site,
            private_mode,
        })
    }

    /// Classify a request origin against this top-level site.
    pub fn classify(&self, request: Option<&Origin>) -> Party {
        classify_party(self.top_level_origin.as_ref(), request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_from_url_and_classifies() {
        let ctx = SiteContext::for_url("https://www.example.com/page", false).unwrap();
        assert_eq!(ctx.top_level_site, "example.com");
        assert!(!ctx.private_mode);
        let other = Origin::parse("https://ads.network.com/").unwrap();
        assert_eq!(ctx.classify(Some(&other)), Party::Third);
        let same = Origin::parse("https://static.example.com/").unwrap();
        assert_eq!(ctx.classify(Some(&same)), Party::First);
    }

    #[test]
    fn empty_context_has_no_origin() {
        let ctx = SiteContext::empty(true);
        assert!(ctx.top_level_origin.is_none());
        assert!(ctx.private_mode);
        assert_eq!(ctx.classify(None), Party::First);
    }

    #[test]
    fn rejects_non_web_urls() {
        assert!(SiteContext::for_url("about:blank", false).is_err());
    }
}
