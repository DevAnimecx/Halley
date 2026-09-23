//! Registry of tools Jerry is allowed to know about.
//!
//! Registration is an allowlist: nothing outside the registry is offered
//! to the model. Prompt #5 does not execute tools.

use std::collections::BTreeMap;

use crate::tool::{RiskLevel, ToolDefinition, ToolId};

/// Allowlist of tool definitions keyed by [`ToolId`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolRegistry {
    tools: BTreeMap<String, ToolDefinition>,
}

impl ToolRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registry pre-populated with [`crate::read_only_tools`].
    pub fn with_read_only_tools() -> Self {
        let mut reg = Self::new();
        for tool in crate::read_only_tools() {
            reg.register(tool);
        }
        reg
    }

    /// Insert or replace a definition. Returns the previous value if any.
    pub fn register(&mut self, definition: ToolDefinition) -> Option<ToolDefinition> {
        self.tools
            .insert(definition.id.as_str().to_string(), definition)
    }

    /// Look up by id.
    pub fn get(&self, id: &ToolId) -> Option<&ToolDefinition> {
        self.tools.get(id.as_str())
    }

    /// All definitions sorted by id (BTreeMap order).
    pub fn iter(&self) -> impl Iterator<Item = &ToolDefinition> {
        self.tools.values()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether no tools are registered.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// True when every registered tool is at or below `max_risk` on the
    /// ReadOnly → Interaction → Sensitive → Destructive ordering
    /// (Forbidden always fails).
    pub fn all_within_risk(&self, max_risk: RiskLevel) -> bool {
        let budget = risk_rank(max_risk);
        self.tools.values().all(|t| {
            let r = risk_rank(t.risk);
            t.risk != RiskLevel::Forbidden && r <= budget
        })
    }
}

fn risk_rank(level: RiskLevel) -> u8 {
    match level {
        RiskLevel::ReadOnly => 0,
        RiskLevel::Interaction => 1,
        RiskLevel::Sensitive => 2,
        RiskLevel::Destructive => 3,
        RiskLevel::Forbidden => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::read_only_tools;

    #[test]
    fn read_only_registry_is_non_empty_and_safe() {
        let reg = ToolRegistry::with_read_only_tools();
        assert!(reg.len() >= 3);
        assert!(reg.all_within_risk(RiskLevel::ReadOnly));
        assert!(reg.get(&ToolId::new("get-current-tab")).is_some());
    }

    #[test]
    fn register_replaces_same_id() {
        let mut reg = ToolRegistry::new();
        let mut tools = read_only_tools();
        let first = tools.pop().unwrap();
        let id = first.id.clone();
        reg.register(first.clone());
        let mut second = first.clone();
        second.description = "updated".into();
        let prev = reg.register(second.clone()).unwrap();
        assert_eq!(prev.description, first.description);
        assert_eq!(reg.get(&id).unwrap().description, "updated");
    }

    #[test]
    fn empty_registry_allows_nothing_useful() {
        let reg = ToolRegistry::new();
        assert!(reg.is_empty());
        assert!(reg.all_within_risk(RiskLevel::ReadOnly));
    }

    #[test]
    fn forbidden_tool_fails_risk_budget() {
        let mut reg = ToolRegistry::new();
        reg.register(ToolDefinition {
            id: ToolId::new("wipe-all"),
            description: "never".into(),
            risk: RiskLevel::Forbidden,
            input_schema: "{}".into(),
        });
        assert!(!reg.all_within_risk(RiskLevel::Destructive));
    }
}
