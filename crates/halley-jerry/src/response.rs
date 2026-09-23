//! Structured model responses (honest stop reasons; no fake success).

use serde::{Deserialize, Serialize};

use crate::provenance::Provenance;

/// Why generation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Model finished a natural turn.
    EndTurn,
    /// Hit max_tokens / output limit.
    Length,
    /// User or host cancelled.
    Cancelled,
    /// Provider or transport error (details on the error path).
    Error,
    /// Content-filter / provider refusal surfaced as a stop.
    Refusal,
}

/// Result of one successful (or cancelled) completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredResponse {
    /// Assistant text (may be empty on pure refusal).
    pub text: String,
    /// Why it stopped.
    pub stop_reason: StopReason,
    /// Provider kind string that produced this (e.g. `"openai"`).
    pub provider: String,
    /// Model id used.
    pub model: String,
    /// Estimated prompt tokens charged to the budget (approximate).
    pub prompt_tokens_estimate: usize,
    /// Approximate completion tokens.
    pub completion_tokens_estimate: usize,
    /// Context item ids that were included in the prompt.
    pub context_ids: Vec<String>,
}

impl StructuredResponse {
    /// Empty cancelled response (used when cancel fires before any delta).
    pub fn cancelled(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            stop_reason: StopReason::Cancelled,
            provider: provider.into(),
            model: model.into(),
            prompt_tokens_estimate: 0,
            completion_tokens_estimate: 0,
            context_ids: Vec::new(),
        }
    }

    /// Provenance for the assistant message derived from this response.
    pub fn provenance(&self) -> Provenance {
        Provenance::Model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_helper_sets_reason() {
        let r = StructuredResponse::cancelled("openai", "gpt-test");
        assert_eq!(r.stop_reason, StopReason::Cancelled);
        assert!(r.text.is_empty());
        assert_eq!(r.provenance(), Provenance::Model);
    }

    #[test]
    fn stop_reason_serde() {
        assert_eq!(
            serde_json::to_string(&StopReason::EndTurn).unwrap(),
            "\"end_turn\""
        );
    }
}
