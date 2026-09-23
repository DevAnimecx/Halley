//! Typed network errors — safe for UI, never contain secrets/bodies.

use std::fmt;

/// Failures from the network pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NetworkError {
    /// URL failed to parse or is not an allowed scheme.
    InvalidUrl,
    /// DNS resolution failed (message without query names if sensitive).
    DnsFailure,
    /// TCP connect / handshake failed.
    ConnectionFailure,
    /// TLS handshake or certificate validation failed.
    TlsFailure,
    /// Request exceeded configured timeout (which phase, not the URL query).
    Timeout {
        /// Which timeout fired.
        phase: TimeoutPhase,
    },
    /// Too many redirects or a forbidden downgrade.
    RedirectLimitExceeded,
    /// Policy blocked the request or redirect.
    RequestBlocked {
        /// Short machine-readable reason (not page content).
        reason: String,
    },
    /// Response status/headers/body framing was invalid.
    ResponseInvalid,
    /// Underlying engine/platform network error (message already sanitized).
    EngineNetworkError(String),
    /// No transport configured (default for Halley-originated requests).
    NoTransport,
}

/// Which timeout fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutPhase {
    /// Connect/handshake.
    Connect,
    /// Full request.
    Request,
    /// Idle connection.
    Idle,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::InvalidUrl => write!(f, "invalid URL"),
            NetworkError::DnsFailure => write!(f, "DNS resolution failed"),
            NetworkError::ConnectionFailure => write!(f, "connection failed"),
            NetworkError::TlsFailure => write!(f, "TLS validation failed"),
            NetworkError::Timeout { phase } => write!(f, "network timeout ({phase:?})"),
            NetworkError::RedirectLimitExceeded => write!(f, "redirect limit exceeded"),
            NetworkError::RequestBlocked { reason } => write!(f, "request blocked: {reason}"),
            NetworkError::ResponseInvalid => write!(f, "invalid response"),
            NetworkError::EngineNetworkError(message) => {
                write!(f, "engine network error: {message}")
            }
            NetworkError::NoTransport => write!(f, "no network transport configured"),
        }
    }
}

impl std::error::Error for NetworkError {}

impl From<NetworkError> for halley_common::Error {
    fn from(err: NetworkError) -> Self {
        halley_common::Error::subsystem(crate::SUBSYSTEM, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_never_contains_placeholder_secrets() {
        let err = NetworkError::RequestBlocked {
            reason: "policy".into(),
        };
        assert_eq!(err.to_string(), "request blocked: policy");
        assert!(NetworkError::NoTransport.to_string().contains("transport"));
    }

    #[test]
    fn converts_to_common_error() {
        let shared: halley_common::Error = NetworkError::InvalidUrl.into();
        assert!(shared.to_string().contains("halley-network"));
    }
}
