//! Shared error strategy.
//!
//! Design goals:
//!
//! * Keep it small — no proc-macro error framework in the foundation.
//! * Let each subsystem define its own specific error enums later, and map
//!   into [`Error`] only at shared boundaries (configuration loading,
//!   orchestration) so crates stay decoupled.
//! * Every error is safe to display: error text must never embed secrets
//!   (API keys, tokens, stored credentials).

use std::fmt;

/// Convenience alias used across the workspace for shared operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that cross subsystem boundaries.
///
/// Subsystems should prefer their own concrete error types internally and
/// convert into [`Error`] only where a shared interface requires it.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A failure that already carries a subsystem name and human-readable
    /// message. Use when a concrete error enum would be overkill at the
    /// boundary.
    Subsystem {
        /// Static name of the reporting subsystem (e.g. `"halley-network"`).
        subsystem: &'static str,
        /// Human-readable description. Must not contain secrets.
        message: String,
    },
    /// Configuration is missing, malformed, or invalid.
    Config(String),
    /// An I/O failure encountered by shared code.
    Io(std::io::Error),
}

impl Error {
    /// Create a subsystem-scoped error with a human-readable message.
    ///
    /// The message must never contain secrets.
    pub fn subsystem(subsystem: &'static str, message: impl Into<String>) -> Self {
        Error::Subsystem {
            subsystem,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Subsystem { subsystem, message } => {
                write!(f, "[{subsystem}] {message}")
            }
            Error::Config(message) => write!(f, "configuration error: {message}"),
            Error::Io(err) => write!(f, "i/o error: {err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn subsystem_error_formats_with_name() {
        let err = Error::subsystem("halley-network", "connection refused");
        assert_eq!(err.to_string(), "[halley-network] connection refused");
    }

    #[test]
    fn config_error_formats_clearly() {
        let err = Error::Config("missing `homepage`".into());
        assert_eq!(err.to_string(), "configuration error: missing `homepage`");
    }

    #[test]
    fn io_error_preserves_source() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let err: Error = io.into();
        assert!(err.source().is_some());
        assert!(err.to_string().contains("no such file"));
    }
}
