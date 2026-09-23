//! Core-level error type.
//!
//! Concrete subsystem errors ([`halley_engine::EngineError`],
//! [`NavigationError`]) are converted here at the orchestration boundary.

use std::fmt;

use crate::navigation::NavigationError;

/// Failures from application orchestration.
#[derive(Debug)]
#[non_exhaustive]
pub enum CoreError {
    /// The engine rejected a command or failed to create views.
    Engine(halley_engine::EngineError),
    /// Address-bar / homepage input could not be turned into a URL.
    Navigation(NavigationError),
    /// The native window or event loop could not be created.
    Window(String),
    /// Configuration is invalid for this run.
    Config(String),
    /// Session-level rejection (lifecycle, missing tab set, snapshot).
    Session(String),
    /// Tab-level failure (unknown id, page/tab desync).
    Tab(String),
    /// Privacy subsystem failure (cookies, storage, profile).
    Privacy(halley_privacy::PrivacyError),
    /// Network pipeline failure.
    Network(halley_network::NetworkError),
    /// Jerry intelligence layer failure (provider, credentials, runtime).
    Jerry(halley_jerry::JerryError),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Engine(err) => write!(f, "{err}"),
            CoreError::Navigation(err) => write!(f, "{err}"),
            CoreError::Window(message) => write!(f, "window error: {message}"),
            CoreError::Config(message) => write!(f, "configuration error: {message}"),
            CoreError::Session(message) => write!(f, "session error: {message}"),
            CoreError::Tab(message) => write!(f, "tab error: {message}"),
            CoreError::Privacy(err) => write!(f, "{err}"),
            CoreError::Network(err) => write!(f, "{err}"),
            CoreError::Jerry(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for CoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CoreError::Engine(err) => Some(err),
            CoreError::Navigation(err) => Some(err),
            CoreError::Privacy(err) => Some(err),
            CoreError::Network(err) => Some(err),
            CoreError::Jerry(err) => Some(err),
            _ => None,
        }
    }
}

impl From<halley_engine::EngineError> for CoreError {
    fn from(err: halley_engine::EngineError) -> Self {
        CoreError::Engine(err)
    }
}

impl From<halley_jerry::JerryError> for CoreError {
    fn from(err: halley_jerry::JerryError) -> Self {
        CoreError::Jerry(err)
    }
}

impl From<NavigationError> for CoreError {
    fn from(err: NavigationError) -> Self {
        CoreError::Navigation(err)
    }
}

impl From<halley_privacy::PrivacyError> for CoreError {
    fn from(err: halley_privacy::PrivacyError) -> Self {
        CoreError::Privacy(err)
    }
}

impl From<halley_privacy::StorageError> for CoreError {
    fn from(err: halley_privacy::StorageError) -> Self {
        CoreError::Privacy(halley_privacy::PrivacyError::from(err))
    }
}

impl From<halley_network::NetworkError> for CoreError {
    fn from(err: halley_network::NetworkError) -> Self {
        CoreError::Network(err)
    }
}

impl From<CoreError> for halley_common::Error {
    fn from(err: CoreError) -> Self {
        halley_common::Error::subsystem(crate::SUBSYSTEM, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn navigation_errors_convert_with_source() {
        let err: CoreError = NavigationError::UnsupportedScheme("javascript".into()).into();
        assert!(err.to_string().contains("javascript"));
        assert!(err.source().is_some());
    }

    #[test]
    fn converts_into_shared_subsystem_error() {
        let shared: halley_common::Error = CoreError::Window("no display".into()).into();
        assert_eq!(shared.to_string(), "[halley-core] window error: no display");
    }
}
