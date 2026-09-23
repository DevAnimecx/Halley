//! Token optimization and model routing.
//!
//! Prompt #5 scope: **context budgeting** only — estimate token cost of
//! candidate context and prioritize what fits. Multi-model routing,
//! prompt compaction, and adaptive strategies are **NOT IMPLEMENTED**.
//!
//! Dependencies: `halley-common` only (workspace graph: jerry → opt → common).

pub mod budget;

pub use budget::{
    estimate_tokens, prioritize, BudgetReport, BudgetedContext, ContextCandidate, Priority,
    DEFAULT_CONTEXT_MAX_TOKENS,
};

/// Identifier for this subsystem, used by diagnostics and logging.
pub const SUBSYSTEM: &str = "halley-jerry-opt";
