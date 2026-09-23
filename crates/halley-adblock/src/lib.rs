//! Advertisement and tracker filtering.
//!
//! `halley-adblock` will evaluate network requests against filter lists and
//! decide allow/block. It is designed to sit behind the
//! [`halley-network`](../../halley-network) request pipeline so filtering is
//! network-level, not script-level.
//!
//! Genesis status: **not implemented**. No filter lists are loaded and no
//! requests are evaluated.

/// Identifier for this subsystem, used by diagnostics and logging.
pub const SUBSYSTEM: &str = "halley-adblock";
