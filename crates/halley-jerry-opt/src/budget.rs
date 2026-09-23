//! Token estimation and context prioritization under a hard budget.
//!
//! Estimation is deliberately approximate (chars/4) so we never claim
//! provider-specific tokenizer accuracy. Callers must treat results as
//! upper-bound-ish heuristics, not billing numbers.

/// Conservative default budget when no config value is available.
pub const DEFAULT_CONTEXT_MAX_TOKENS: usize = 4_096;

/// Relative importance of a context candidate when trimming to budget.
///
/// Higher priority is kept first when space runs out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// System / safety framing — keep at all costs.
    Critical = 3,
    /// Current user turn.
    High = 2,
    /// Browser state (url/title/tab) — usually small and useful.
    Medium = 1,
    /// Page/webpage text and lower-value history — dropped first.
    Low = 0,
}

impl Priority {
    /// Numeric rank for sorting (higher first).
    pub fn rank(self) -> u8 {
        self as u8
    }
}

/// One unit of potential prompt context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextCandidate {
    /// Stable id for reporting which items were kept/dropped.
    pub id: String,
    /// Priority used when trimming.
    pub priority: Priority,
    /// Raw text as it would appear in the prompt.
    pub text: String,
}

impl ContextCandidate {
    /// Build a candidate.
    pub fn new(id: impl Into<String>, priority: Priority, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            priority,
            text: text.into(),
        }
    }
}

/// Rough token estimate for free text (≈ chars/4, min 0).
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

/// Result of fitting candidates into a budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetReport {
    /// Tokens estimated for kept items (plus optional reserved tokens).
    pub kept_tokens: usize,
    /// Ids kept, in original relative priority order (stable within tier).
    pub kept: Vec<String>,
    /// Ids dropped because they did not fit.
    pub dropped: Vec<String>,
    /// True when every candidate fit.
    pub all_fit: bool,
}

/// Kept text ready to splice into a prompt, in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetedContext {
    /// Concatenated kept texts (caller chooses separator).
    pub texts: Vec<String>,
    /// Report describing what was dropped.
    pub report: BudgetReport,
}

/// Fit `candidates` into `max_tokens`.
///
/// Order: higher [`Priority`] first; within the same priority, original
/// order is preserved (stable). Items that do not fit are dropped — we
/// never truncate mid-item (partial context is harder to reason about).
///
/// `reserve` tokens are left free (e.g. for the model reply headroom);
/// only `max_tokens.saturating_sub(reserve)` is available for candidates.
pub fn prioritize(
    candidates: &[ContextCandidate],
    max_tokens: usize,
    reserve: usize,
) -> BudgetedContext {
    let available = max_tokens.saturating_sub(reserve);

    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|&a, &b| {
        candidates[b]
            .priority
            .rank()
            .cmp(&candidates[a].priority.rank())
            .then(a.cmp(&b))
    });

    let mut kept_tokens = 0usize;
    let mut kept_flags = vec![false; candidates.len()];
    let mut dropped = Vec::new();

    for idx in order {
        let c = &candidates[idx];
        let cost = estimate_tokens(&c.text);
        if kept_tokens + cost <= available {
            kept_tokens += cost;
            kept_flags[idx] = true;
        } else {
            dropped.push(c.id.clone());
        }
    }

    // Report kept in original candidate order (stable for callers).
    let mut kept = Vec::new();
    let mut texts = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        if kept_flags[i] {
            kept.push(c.id.clone());
            texts.push(c.text.clone());
        }
    }

    let all_fit = dropped.is_empty();
    BudgetedContext {
        texts,
        report: BudgetReport {
            kept_tokens,
            kept,
            dropped,
            all_fit,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_is_quarters_ceil() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(estimate_tokens("日").max(1), 1);
    }

    #[test]
    fn high_priority_dropped_last() {
        let low = ContextCandidate::new("page", Priority::Low, "x".repeat(800));
        let high = ContextCandidate::new("user", Priority::High, "y".repeat(800));
        // 800 chars ≈ 200 tokens each; budget 200 keeps only one (high).
        let out = prioritize(&[low, high], 200, 0);
        assert!(out.report.kept.contains(&"user".to_string()));
        assert!(out.report.dropped.contains(&"page".to_string()));
        assert!(!out.report.all_fit);
    }

    #[test]
    fn everything_fits_within_budget() {
        let a = ContextCandidate::new("a", Priority::Medium, "hello");
        let b = ContextCandidate::new("b", Priority::Low, "world");
        let out = prioritize(&[a, b], 1_000, 0);
        assert!(out.report.all_fit);
        assert_eq!(out.texts.len(), 2);
    }

    #[test]
    fn reserve_reduces_available_space() {
        let big = ContextCandidate::new("big", Priority::Critical, "z".repeat(64));
        // 64 chars ≈ 16 tokens; with reserve 16 available is 0.
        let out = prioritize(&[big], 16, 16);
        assert!(out.report.dropped.contains(&"big".to_string()));
    }

    #[test]
    fn same_priority_keeps_original_order() {
        let a = ContextCandidate::new("a", Priority::Low, "aa");
        let b = ContextCandidate::new("b", Priority::Low, "bb");
        let out = prioritize(&[a, b], 1_000, 0);
        assert_eq!(out.report.kept, vec!["a".to_string(), "b".to_string()]);
    }
}
