//! Browser context items and budgeted bundles for prompts.

use halley_jerry_opt::{estimate_tokens, prioritize, ContextCandidate, Priority};

use crate::provenance::Provenance;

/// Snapshot of browser state core passes into Jerry (no engine dependency).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrowserPageContext {
    /// Active tab id (`None` if no tab).
    pub tab_id: Option<u64>,
    /// Active tab URL (may be `about:blank`).
    pub url: String,
    /// Active tab title (may be empty).
    pub title: String,
    /// Whether the session is private browsing.
    pub private_mode: bool,
    /// Number of open tabs.
    pub open_tab_count: usize,
}

/// One labeled context contribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItem {
    /// Id used in budget reports / structured response.
    pub id: String,
    /// Who produced this text.
    pub provenance: Provenance,
    /// Trim priority.
    pub priority: Priority,
    /// Text body (unframed; framing happens in privacy pipeline).
    pub text: String,
}

impl ContextItem {
    /// Construct.
    pub fn new(
        id: impl Into<String>,
        provenance: Provenance,
        priority: Priority,
        text: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provenance,
            priority,
            text: text.into(),
        }
    }
}

/// Builds [`ContextItem`]s from browser state (metadata only — no DOM text).
#[derive(Debug, Default, Clone, Copy)]
pub struct PageContextProvider;

impl PageContextProvider {
    /// Create provider.
    pub fn new() -> Self {
        Self
    }

    /// Browser-state items for the active page (always small).
    pub fn from_browser(&self, browser: &BrowserPageContext) -> Vec<ContextItem> {
        let mut items = Vec::new();
        let url = browser.url.trim();
        if !url.is_empty() {
            items.push(ContextItem::new(
                "browser.url",
                Provenance::Browser,
                Priority::Medium,
                format!("Active tab URL: {url}"),
            ));
        }
        let title = browser.title.trim();
        if !title.is_empty() {
            items.push(ContextItem::new(
                "browser.title",
                Provenance::Browser,
                Priority::Medium,
                format!("Active tab title: {title}"),
            ));
        }
        items.push(ContextItem::new(
            "browser.tabs",
            Provenance::Browser,
            Priority::Low,
            format!(
                "Open tabs: {}{}",
                browser.open_tab_count,
                if browser.private_mode {
                    " (private session)"
                } else {
                    ""
                }
            ),
        ));
        if let Some(id) = browser.tab_id {
            items.push(ContextItem::new(
                "browser.tab_id",
                Provenance::Browser,
                Priority::Low,
                format!("Active tab id: {id}"),
            ));
        }
        items
    }
}

/// Budgeted context ready to format into the prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBundle {
    /// Kept items in priority-stable display order (already budgeted).
    pub items: Vec<ContextItem>,
    /// Estimated tokens for `items`.
    pub tokens_estimate: usize,
    /// Ids dropped by the budget.
    pub dropped_ids: Vec<String>,
}

impl ContextBundle {
    /// Fit items into `max_tokens` leaving `reserve` free (reply headroom).
    pub fn budget(items: Vec<ContextItem>, max_tokens: usize, reserve: usize) -> Self {
        let candidates: Vec<ContextCandidate> = items
            .iter()
            .map(|i| ContextCandidate::new(i.id.clone(), i.priority, i.text.clone()))
            .collect();
        let budgeted = prioritize(&candidates, max_tokens, reserve);
        let dropped: std::collections::HashSet<&str> =
            budgeted.report.dropped.iter().map(String::as_str).collect();
        let kept_items: Vec<ContextItem> = items
            .into_iter()
            .filter(|i| !dropped.contains(i.id.as_str()))
            .collect();
        let tokens_estimate = kept_items.iter().map(|i| estimate_tokens(&i.text)).sum();
        ContextBundle {
            items: kept_items,
            tokens_estimate,
            dropped_ids: budgeted.report.dropped,
        }
    }

    /// Render items as a single context block for the prompt.
    pub fn render(&self) -> String {
        self.items
            .iter()
            .map(|i| format!("[{}]\n{}", i.provenance.as_str(), i.text))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Ids included (for structured response).
    pub fn ids(&self) -> Vec<String> {
        self.items.iter().map(|i| i.id.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_metadata_items_are_present() {
        let browser = BrowserPageContext {
            tab_id: Some(1),
            url: "https://example.com/".into(),
            title: "Example".into(),
            private_mode: false,
            open_tab_count: 3,
        };
        let items = PageContextProvider::new().from_browser(&browser);
        let ids: Vec<_> = items.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"browser.url"));
        assert!(ids.contains(&"browser.title"));
        assert!(ids.contains(&"browser.tabs"));
        assert!(items.iter().all(|i| i.provenance == Provenance::Browser));
    }

    #[test]
    fn empty_url_title_omitted() {
        let browser = BrowserPageContext {
            open_tab_count: 1,
            ..Default::default()
        };
        let items = PageContextProvider::new().from_browser(&browser);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "browser.tabs");
    }

    #[test]
    fn budget_drops_low_priority_first() {
        let items = vec![
            ContextItem::new(
                "critical",
                Provenance::System,
                Priority::Critical,
                "keep me",
            ),
            ContextItem::new(
                "huge_page",
                Provenance::Webpage,
                Priority::Low,
                "x".repeat(4000),
            ),
        ];
        let bundle = ContextBundle::budget(items, 100, 0);
        assert!(bundle.ids().contains(&"critical".to_string()));
        assert!(bundle.dropped_ids.contains(&"huge_page".to_string()));
    }

    #[test]
    fn render_labels_provenance() {
        let items = vec![ContextItem::new(
            "browser.url",
            Provenance::Browser,
            Priority::Medium,
            "Active tab URL: https://example.com/",
        )];
        let bundle = ContextBundle::budget(items, 1000, 0);
        let text = bundle.render();
        assert!(text.contains("[browser]"));
        assert!(text.contains("https://example.com/"));
    }

    #[test]
    fn private_mode_noted() {
        let browser = BrowserPageContext {
            private_mode: true,
            open_tab_count: 2,
            ..Default::default()
        };
        let items = PageContextProvider::new().from_browser(&browser);
        let tabs = items.iter().find(|i| i.id == "browser.tabs").unwrap();
        assert!(tabs.text.contains("private session"));
    }
}
