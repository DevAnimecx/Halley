//! Provenance labels for every piece of context entering Jerry/LLM.
//!
//! Page content and tool output are **data**, never instructions. The
//! provenance tag is how the privacy pipeline frames untrusted material.

use serde::{Deserialize, Serialize};

/// Who produced a message or context item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Typed by the human user in the chat UI.
    User,
    /// Fixed product/system instructions from Halley (not model output).
    System,
    /// Halley application chrome / configuration framing.
    Halley,
    /// Content from a webpage (untrusted).
    Webpage,
    /// Browser state (tab url/title) observed by core.
    Browser,
    /// Output from the LLM provider.
    Model,
    /// Output of a browser tool call.
    Tool,
}

impl Provenance {
    /// Whether material with this provenance must be wrapped as untrusted
    /// data (never treated as instructions).
    pub fn is_untrusted(self) -> bool {
        matches!(self, Provenance::Webpage | Provenance::Tool)
    }

    /// Stable label used in framing tags and tests.
    pub fn as_str(self) -> &'static str {
        match self {
            Provenance::User => "user",
            Provenance::System => "system",
            Provenance::Halley => "halley",
            Provenance::Webpage => "webpage",
            Provenance::Browser => "browser",
            Provenance::Model => "model",
            Provenance::Tool => "tool",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webpage_and_tool_are_untrusted() {
        assert!(Provenance::Webpage.is_untrusted());
        assert!(Provenance::Tool.is_untrusted());
        assert!(!Provenance::User.is_untrusted());
        assert!(!Provenance::System.is_untrusted());
        assert!(!Provenance::Browser.is_untrusted());
    }

    #[test]
    fn serde_round_trip() {
        let json = serde_json::to_string(&Provenance::Webpage).unwrap();
        assert_eq!(json, "\"webpage\"");
        let back: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Provenance::Webpage);
    }
}
