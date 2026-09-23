//! Built-in tool definitions available in Prompt #5.
//!
//! Only observation tools. Schemas are JSON Schema strings embedded as
//! constants so the model-facing description and tests share one source.

use crate::tool::{RiskLevel, ToolDefinition, ToolId};

/// `getCurrentTab` — active tab id and display title (browser state only).
pub fn get_current_tab() -> ToolDefinition {
    ToolDefinition {
        id: ToolId::new("get-current-tab"),
        description: "Return the active tab id and title. Read-only.".into(),
        risk: RiskLevel::ReadOnly,
        input_schema: r#"{"type":"object","properties":{},"additionalProperties":false}"#.into(),
    }
}

/// `getCurrentPageMetadata` — active URL/title; **not** full page text.
pub fn get_current_page_metadata() -> ToolDefinition {
    ToolDefinition {
        id: ToolId::new("get-current-page-metadata"),
        description: "Return the active tab URL and title. Does not return page body text.".into(),
        risk: RiskLevel::ReadOnly,
        input_schema: r#"{"type":"object","properties":{},"additionalProperties":false}"#.into(),
    }
}

/// `getSelectedText` — selected text in the page when the engine can
/// provide it; otherwise the executor (future) returns `unavailable`.
pub fn get_selected_text() -> ToolDefinition {
    ToolDefinition {
        id: ToolId::new("get-selected-text"),
        description: "Return text selected in the active page, if the engine exposes it.".into(),
        risk: RiskLevel::ReadOnly,
        input_schema: r#"{"type":"object","properties":{},"additionalProperties":false}"#.into(),
    }
}

/// All read-only tools registered in Prompt #5.
pub fn read_only_tools() -> Vec<ToolDefinition> {
    vec![
        get_current_tab(),
        get_current_page_metadata(),
        get_selected_text(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::RiskLevel;

    #[test]
    fn all_builtin_tools_are_read_only() {
        for tool in read_only_tools() {
            assert_eq!(tool.risk, RiskLevel::ReadOnly, "{}", tool.id);
            assert!(!tool.input_schema.is_empty());
            assert!(serde_json::from_str::<serde_json::Value>(&tool.input_schema).is_ok());
        }
    }

    #[test]
    fn tool_ids_are_unique() {
        let tools = read_only_tools();
        let mut names: Vec<_> = tools.iter().map(|t| t.id.as_str().to_owned()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), tools.len());
    }
}
