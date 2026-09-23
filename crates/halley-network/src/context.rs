//! Request privacy context: which tab/page/site and private mode.

use halley_common::Origin;

/// Resource class for future filtering and policy (not a full classifier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceType {
    /// Top-level document navigation.
    Document,
    Script,
    Image,
    Stylesheet,
    Font,
    Media,
    /// XHR / fetch.
    Xhr,
    WebSocket,
    /// Anything else.
    #[default]
    Other,
}

/// Context attached to every request for privacy decisions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RequestContext {
    /// Browser tab that initiated the request, when known.
    pub tab_id: Option<u64>,
    /// Engine page that initiated the request, when known.
    pub page_id: Option<u64>,
    /// Top-level document origin (None on about: pages).
    pub top_level_origin: Option<Origin>,
    /// Origin of the requesting subresource / script, when known.
    pub request_origin: Option<Origin>,
    /// Destination origin of this request.
    pub destination: Option<Origin>,
    /// Resource classification.
    pub resource_type: ResourceType,
    /// Whether the initiating session is private browsing.
    pub private_mode: bool,
}

impl RequestContext {
    /// Context for a top-level document load.
    pub fn document(url: &str, private_mode: bool) -> Self {
        let origin = Origin::parse(url).ok();
        RequestContext {
            top_level_origin: origin.clone(),
            destination: origin,
            resource_type: ResourceType::Document,
            private_mode,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_context_sets_origin_and_private_flag() {
        let ctx = RequestContext::document("https://example.com/page", true);
        assert_eq!(
            ctx.top_level_origin.as_ref().map(|o| o.host()),
            Some("example.com")
        );
        assert!(ctx.private_mode);
        assert_eq!(ctx.resource_type, ResourceType::Document);
    }

    #[test]
    fn about_blank_has_no_origin() {
        let ctx = RequestContext::document("about:blank", false);
        assert!(ctx.top_level_origin.is_none());
    }
}
