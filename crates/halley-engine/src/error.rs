//! Engine-boundary error type.
//!
//! Concrete backends map their native errors into [`EngineError`] at the
//! seam so `halley-core` never names wry types (except through the
//! optional `native` constructor).

use std::fmt;

/// Failures surfaced by a [`crate::BrowserEngine`] implementation.
#[derive(Debug)]
#[non_exhaustive]
pub enum EngineError {
    /// Creating the native web views or their host window attachment failed.
    Create(String),
    /// A navigation command was rejected by the engine.
    Navigate(String),
    /// A chrome (UI) update could not be applied.
    ChromeUpdate(String),
    /// Layout / bounds change failed.
    Layout(String),
    /// Focusing the content view failed.
    Focus(String),
    /// The engine is in a state where the command cannot run (e.g. no
    /// history entry in that direction).
    InvalidState(String),
    /// The page id does not exist in this engine instance.
    PageNotFound(u64),
    /// The operation is not supported by this engine (e.g. stop on wry).
    Unsupported(&'static str),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Create(message) => write!(f, "engine create failed: {message}"),
            EngineError::Navigate(message) => write!(f, "navigation failed: {message}"),
            EngineError::ChromeUpdate(message) => write!(f, "chrome update failed: {message}"),
            EngineError::Layout(message) => write!(f, "layout failed: {message}"),
            EngineError::Focus(message) => write!(f, "focus failed: {message}"),
            EngineError::InvalidState(message) => write!(f, "invalid engine state: {message}"),
            EngineError::PageNotFound(id) => write!(f, "page not found: {id}"),
            EngineError::Unsupported(what) => write!(f, "unsupported operation: {what}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<EngineError> for halley_common::Error {
    fn from(err: EngineError) -> Self {
        halley_common::Error::subsystem(crate::SUBSYSTEM, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_are_clear_and_free_of_secrets_by_construction() {
        let err = EngineError::Navigate("blocked scheme".into());
        assert_eq!(err.to_string(), "navigation failed: blocked scheme");
    }

    #[test]
    fn page_not_found_and_unsupported_format() {
        assert_eq!(
            EngineError::PageNotFound(7).to_string(),
            "page not found: 7"
        );
        assert_eq!(
            EngineError::Unsupported("stop").to_string(),
            "unsupported operation: stop"
        );
    }

    #[test]
    fn converts_into_shared_subsystem_error() {
        let shared: halley_common::Error = EngineError::Create("no webview".into()).into();
        assert_eq!(
            shared.to_string(),
            "[halley-engine] engine create failed: no webview"
        );
    }
}
