//! Tool identity, risk classification, and call/result types.
//!
//! Risk tiers match ADR-005 / security model. Prompt #5 only registers
//! [`RiskLevel::ReadOnly`] tools; higher tiers exist so the enum is stable
//! before interactive tools land.

use serde::{Deserialize, Serialize};

/// Stable tool identifier (kebab-case name used in schemas and logs).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId(String);

impl ToolId {
    /// Construct from a known-good name (builders use constants).
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Tool name string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ToolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Safety classification enforced on the **browser side** (never model-side).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskLevel {
    /// Observation only; no side effects. Prompt #5 only.
    ReadOnly,
    /// UI interaction (click, type) — not implemented yet.
    Interaction,
    /// Touches secrets/permissions/cookies — not implemented yet.
    Sensitive,
    /// Destructive; requires explicit user confirmation — not implemented.
    Destructive,
    /// Always refused regardless of model output.
    Forbidden,
}

/// One tool invocation proposed by the model (not yet executed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Which tool.
    pub id: ToolId,
    /// JSON-encoded arguments (object string). Callers parse; we keep a
    /// string so untrusted model output is not structurally trusted early.
    pub arguments: String,
}

/// Tool execution outcome (for contracts / future executor).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    /// Tool that produced this result.
    pub id: ToolId,
    /// Outcome status.
    pub status: ToolStatus,
    /// JSON payload or error detail (never secrets).
    pub payload: String,
}

/// Status of a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    /// Completed successfully.
    Ok,
    /// Rejected by risk policy or confirmation gate.
    Denied,
    /// Tool or feature not available in this build.
    Unavailable,
    /// Execution failed (message in payload).
    Error,
}

/// Static description of a tool Jerry may be told about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Stable name.
    pub id: ToolId,
    /// One-line human description for model/system prompts.
    pub description: String,
    /// Risk tier (enforced by browser when an executor exists).
    pub risk: RiskLevel,
    /// JSON Schema (as a JSON string) for `arguments`.
    pub input_schema: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_id_round_trips_serde() {
        let id = ToolId::new("get-current-tab");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"get-current-tab\"");
        let back: ToolId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn risk_level_serde_is_upper_snake() {
        assert_eq!(
            serde_json::to_string(&RiskLevel::ReadOnly).unwrap(),
            "\"READ_ONLY\""
        );
    }

    #[test]
    fn tool_call_keeps_arguments_as_string() {
        let call = ToolCall {
            id: ToolId::new("get-current-tab"),
            arguments: "{}".into(),
        };
        assert_eq!(call.arguments, "{}");
    }
}
