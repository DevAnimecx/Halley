//! Development logging foundation.
//!
//! Privacy rules for all logging in Halley:
//!
//! * Logs are written to local stderr only. Nothing is ever sent over the
//!   network.
//! * There is no telemetry and no analytics, and logging must never become a
//!   covert channel for either.
//! * Secrets (API keys, tokens, stored credentials) must never appear in log
//!   statements, error messages, or debug output.
//! * Page content read by Jerry must not be logged at default levels; it is
//!   untrusted, potentially sensitive, and large.
//!
//! The production build will eventually add stricter log filtering and a
//! redaction layer (planned — see `docs/privacy-model.md`). For now this
//! module provides a zero-dependency, local-only logger so development and
//! tests have a single, safe entry point.

use std::sync::OnceLock;

/// Severity of a log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Error conditions.
    Error,
    /// Warnings.
    Warn,
    /// Informational messages.
    Info,
    /// Debug detail.
    Debug,
    /// Verbose tracing detail.
    Trace,
}

impl Level {
    /// Parse a level name (`"error"`, `"warn"`, …, case-insensitive).
    pub fn parse(s: &str) -> Option<Level> {
        match s.to_ascii_lowercase().as_str() {
            "error" => Some(Level::Error),
            "warn" => Some(Level::Warn),
            "info" => Some(Level::Info),
            "debug" => Some(Level::Debug),
            "trace" => Some(Level::Trace),
            _ => None,
        }
    }

    /// Uppercase name used when writing records.
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }
}

/// Global maximum level; records below this are dropped at the call site.
static MAX_LEVEL: OnceLock<Level> = OnceLock::new();

/// Initialize development logging.
///
/// Reads `HALLEY_LOG` (e.g. `HALLEY_LOG=debug`) to select the maximum level;
/// defaults to `info`. Safe to call more than once — subsequent calls are
/// ignored. Output goes to stderr only.
pub fn init() {
    let level = std::env::var("HALLEY_LOG")
        .ok()
        .and_then(|value| Level::parse(&value))
        .unwrap_or(Level::Info);
    let _ = MAX_LEVEL.set(level);
}

/// The active maximum level (defaults to `info` if [`init`] was never called).
pub fn max_level() -> Level {
    *MAX_LEVEL.get().unwrap_or(&Level::Info)
}

/// Emit a record if `level` is enabled. Writes `[{level}] {message}` to stderr.
///
/// Callers must not pass secrets or page content as `message`.
#[doc(hidden)]
pub fn __log(level: Level, message: std::fmt::Arguments<'_>) {
    if level <= max_level() {
        eprintln!("[{}] {}", level.as_str(), message);
    }
}

/// Log an error-level message.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logging::__log($crate::logging::Level::Error, format_args!($($arg)*))
    };
}

/// Log a warning-level message.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logging::__log($crate::logging::Level::Warn, format_args!($($arg)*))
    };
}

/// Log an info-level message.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logging::__log($crate::logging::Level::Info, format_args!($($arg)*))
    };
}

/// Log a debug-level message.
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::logging::__log($crate::logging::Level::Debug, format_args!($($arg)*))
    };
}

/// Log a trace-level message.
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        $crate::logging::__log($crate::logging::Level::Trace, format_args!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_levels() {
        assert_eq!(Level::parse("error"), Some(Level::Error));
        assert_eq!(Level::parse("WARN"), Some(Level::Warn));
        assert_eq!(Level::parse("info"), Some(Level::Info));
        assert_eq!(Level::parse("debug"), Some(Level::Debug));
        assert_eq!(Level::parse("trace"), Some(Level::Trace));
        assert_eq!(Level::parse("bogus"), None);
    }

    #[test]
    fn default_max_level_is_info() {
        assert_eq!(max_level(), Level::Info);
    }

    #[test]
    fn level_ordering_is_severity_ordered() {
        assert!(Level::Error < Level::Warn);
        assert!(Level::Warn < Level::Info);
        assert!(Level::Info < Level::Debug);
        assert!(Level::Debug < Level::Trace);
    }
}
