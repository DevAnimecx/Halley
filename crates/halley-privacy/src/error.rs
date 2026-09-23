//! Error types for privacy, cookies, storage, and profiles.
//!
//! Messages must never include cookie values, tokens, or full private
//! temp paths beyond what the operator needs (directory basenames are OK).

use std::fmt;

/// Top-level privacy subsystem failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum PrivacyError {
    /// Cookie store / policy failure.
    Cookie(CookieError),
    /// Storage path or filesystem policy failure.
    Storage(StorageError),
    /// Profile create/open/close failure.
    Profile(ProfileError),
}

impl fmt::Display for PrivacyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrivacyError::Cookie(err) => write!(f, "{err}"),
            PrivacyError::Storage(err) => write!(f, "{err}"),
            PrivacyError::Profile(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for PrivacyError {}

impl From<CookieError> for PrivacyError {
    fn from(err: CookieError) -> Self {
        PrivacyError::Cookie(err)
    }
}

impl From<StorageError> for PrivacyError {
    fn from(err: StorageError) -> Self {
        PrivacyError::Storage(err)
    }
}

impl From<ProfileError> for PrivacyError {
    fn from(err: ProfileError) -> Self {
        PrivacyError::Profile(err)
    }
}

impl From<PrivacyError> for halley_common::Error {
    fn from(err: PrivacyError) -> Self {
        halley_common::Error::subsystem(crate::SUBSYSTEM, err.to_string())
    }
}

/// Cookie parsing / matching failures. Never carries cookie values.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CookieError {
    /// Set-Cookie header could not be parsed.
    Malformed,
    /// Cookie had no name.
    MissingName,
    /// Domain attribute was empty or invalid.
    InvalidDomain,
    /// Path attribute was invalid.
    InvalidPath,
    /// Policy rejected storing this cookie in the current context.
    BlockedByPolicy,
    /// Expired cookies are not accepted or returned.
    Expired,
}

impl fmt::Display for CookieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CookieError::Malformed => write!(f, "malformed cookie"),
            CookieError::MissingName => write!(f, "cookie missing name"),
            CookieError::InvalidDomain => write!(f, "invalid cookie domain"),
            CookieError::InvalidPath => write!(f, "invalid cookie path"),
            CookieError::BlockedByPolicy => write!(f, "cookie blocked by policy"),
            CookieError::Expired => write!(f, "cookie expired"),
        }
    }
}

impl std::error::Error for CookieError {}

/// Storage path / directory failures. Paths in messages are roots only.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StorageError {
    /// Component contained path traversal or separators.
    UnsafeComponent,
    /// Joining would escape the storage root.
    PathEscapesRoot,
    /// Filesystem operation failed (message without secrets).
    Io(String),
    /// Category name was not one of the known storage categories.
    UnknownCategory(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::UnsafeComponent => write!(f, "unsafe storage path component"),
            StorageError::PathEscapesRoot => write!(f, "path escapes storage root"),
            StorageError::Io(message) => write!(f, "storage i/o: {message}"),
            StorageError::UnknownCategory(category) => {
                write!(f, "unknown storage category: {category}")
            }
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        // Full OS paths can be long; keep the error kind message only.
        StorageError::Io(err.kind().to_string())
    }
}

/// Profile lifecycle failures.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProfileError {
    /// Could not create or open profile directories.
    CreateFailed,
    /// Close/cleanup failed after a private session.
    CleanupFailed,
    /// Operation not allowed for this profile kind.
    NotAllowed,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::CreateFailed => write!(f, "failed to create profile"),
            ProfileError::CleanupFailed => write!(f, "failed to clean up profile"),
            ProfileError::NotAllowed => write!(f, "operation not allowed for profile kind"),
        }
    }
}

impl std::error::Error for ProfileError {}
