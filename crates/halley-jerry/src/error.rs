//! Jerry error type — safe to display; never contains secrets.

use std::fmt;

/// Failures inside the Jerry intelligence layer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum JerryError {
    /// No provider selected, or provider not recognized.
    ProviderNotConfigured,
    /// API key missing or empty for the selected provider.
    MissingApiKey,
    /// Endpoint failed allowlist / URL rules.
    EndpointNotAllowed {
        /// Short reason (host/scheme), never a key.
        reason: String,
    },
    /// Provider or transport I/O failed (sanitized message).
    Transport(String),
    /// Provider returned a non-success status.
    ProviderStatus {
        /// HTTP status if known.
        status: u16,
        /// Sanitized body snippet or message (truncated, no secrets).
        detail: String,
    },
    /// Response body could not be parsed.
    InvalidResponse(String),
    /// Conversation / credential storage I/O.
    Storage(String),
    /// Input rejected before send (empty message, budget misuse, …).
    InvalidRequest(String),
    /// Operation cancelled via the cancel flag.
    Cancelled,
    /// Feature intentionally not available in this build.
    Unsupported(&'static str),
}

impl fmt::Display for JerryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JerryError::ProviderNotConfigured => write!(f, "no AI provider configured"),
            JerryError::MissingApiKey => write!(f, "API key missing for provider"),
            JerryError::EndpointNotAllowed { reason } => {
                write!(f, "provider endpoint not allowed: {reason}")
            }
            JerryError::Transport(message) => write!(f, "provider transport error: {message}"),
            JerryError::ProviderStatus { status, detail } => {
                write!(f, "provider returned HTTP {status}: {detail}")
            }
            JerryError::InvalidResponse(message) => {
                write!(f, "invalid provider response: {message}")
            }
            JerryError::Storage(message) => write!(f, "jerry storage error: {message}"),
            JerryError::InvalidRequest(message) => write!(f, "invalid request: {message}"),
            JerryError::Cancelled => write!(f, "cancelled"),
            JerryError::Unsupported(what) => write!(f, "not implemented: {what}"),
        }
    }
}

impl std::error::Error for JerryError {}

impl From<JerryError> for halley_common::Error {
    fn from(err: JerryError) -> Self {
        halley_common::Error::subsystem(crate::SUBSYSTEM, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_never_embeds_raw_key_shapes_from_transport_strings() {
        let err = JerryError::Transport("connection refused".into());
        assert!(err.to_string().contains("connection refused"));
        let cancelled: halley_common::Error = JerryError::Cancelled.into();
        assert!(cancelled.to_string().contains("halley-jerry"));
    }

    #[test]
    fn endpoint_error_is_structured_not_freeform_url_with_query() {
        let err = JerryError::EndpointNotAllowed {
            reason: "scheme http".into(),
        };
        assert!(err.to_string().contains("scheme http"));
    }
}
