//! BYOK credential storage for Jerry providers.
//!
//! Prompt #4 provided profile-scoped storage **paths** and path safety —
//! not OS keychain encryption. This store reuses that layout:
//!
//! * Normal profile: JSON file under the profile `profile/` category.
//! * Private / memory-only: no disk writes.
//!
//! API keys are never logged, never placed in URLs, and are redacted in
//! [`std::fmt::Debug`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::JerryError;
use crate::provider::ProviderKind;

/// Schema version for the credentials file.
pub const CREDENTIALS_SCHEMA_VERSION: u32 = 1;

/// Filename under the profile directory (single safe component).
pub const CREDENTIALS_FILE_NAME: &str = "jerry-credentials.json";

/// One provider's stored secret + optional overrides.
#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderCredential {
    /// API key / bearer token. Redacted in Debug.
    pub api_key: String,
    /// Optional custom base URL (must still pass endpoint allowlist).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Optional default model for this provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl ProviderCredential {
    /// New credential with only a key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: None,
            model: None,
        }
    }

    /// Whether the key looks empty after trim.
    pub fn is_empty_key(&self) -> bool {
        self.api_key.trim().is_empty()
    }
}

impl std::fmt::Debug for ProviderCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderCredential")
            .field("api_key", &"REDACTED")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .finish()
    }
}

impl PartialEq for ProviderCredential {
    fn eq(&self, other: &Self) -> bool {
        self.api_key == other.api_key
            && self.base_url == other.base_url
            && self.model == other.model
    }
}

impl Eq for ProviderCredential {}

/// On-disk envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialsFile {
    schema_version: u32,
    providers: BTreeMap<String, ProviderCredential>,
    selected: Option<String>,
}

/// In-memory credential map with optional persistence path.
#[derive(Debug)]
pub struct CredentialStore {
    path: Option<PathBuf>,
    providers: BTreeMap<String, ProviderCredential>,
    selected: Option<String>,
}

impl CredentialStore {
    /// Memory-only store (private profiles / tests).
    pub fn memory_only() -> Self {
        Self {
            path: None,
            providers: BTreeMap::new(),
            selected: None,
        }
    }

    /// Store that loads/saves `path` (normal profile).
    pub fn at_path(path: impl Into<PathBuf>) -> Result<Self, JerryError> {
        let path = path.into();
        let mut store = Self {
            path: Some(path.clone()),
            providers: BTreeMap::new(),
            selected: None,
        };
        if path.exists() {
            store.load()?;
        }
        Ok(store)
    }

    /// Path used for persistence (`None` = memory-only).
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Currently selected provider key (e.g. `"openai"`).
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Set selected provider (clears if unknown and no credential — selection
    /// may still be set before the key is saved).
    pub fn set_selected(&mut self, provider: ProviderKind) {
        self.selected = Some(provider.as_str().to_string());
    }

    /// Store or replace a credential; marks provider selected.
    pub fn put(
        &mut self,
        provider: ProviderKind,
        credential: ProviderCredential,
    ) -> Result<(), JerryError> {
        if credential.is_empty_key() {
            return Err(JerryError::InvalidRequest("empty API key".into()));
        }
        self.providers
            .insert(provider.as_str().to_string(), credential);
        self.selected = Some(provider.as_str().to_string());
        self.save()
    }

    /// Remove a provider credential.
    pub fn remove(&mut self, provider: ProviderKind) -> Result<(), JerryError> {
        self.providers.remove(provider.as_str());
        if self.selected.as_deref() == Some(provider.as_str()) {
            self.selected = None;
        }
        self.save()
    }

    /// Lookup credential for a provider.
    pub fn get(&self, provider: ProviderKind) -> Option<&ProviderCredential> {
        self.providers.get(provider.as_str())
    }

    /// Whether a non-empty key exists for the provider.
    pub fn has_key(&self, provider: ProviderKind) -> bool {
        self.get(provider).is_some_and(|c| !c.is_empty_key())
    }

    /// Redacted summary for UI (never the key).
    pub fn key_hint(&self, provider: ProviderKind) -> Option<String> {
        let cred = self.get(provider)?;
        if cred.is_empty_key() {
            return None;
        }
        let key = cred.api_key.trim();
        let tail = key.chars().rev().take(4).collect::<String>();
        let tail: String = tail.chars().rev().collect();
        Some(format!("…{tail}"))
    }

    fn load(&mut self) -> Result<(), JerryError> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| JerryError::Storage("memory-only store".into()))?;
        let bytes = std::fs::read(path)
            .map_err(|err| JerryError::Storage(format!("read credentials: {}", err.kind())))?;
        let file: CredentialsFile = serde_json::from_slice(&bytes)
            .map_err(|err| JerryError::Storage(format!("parse credentials: {err}")))?;
        if file.schema_version != CREDENTIALS_SCHEMA_VERSION {
            return Err(JerryError::Storage(format!(
                "unsupported credentials schema_version {}",
                file.schema_version
            )));
        }
        self.providers = file.providers;
        self.selected = file.selected;
        Ok(())
    }

    /// Persist if a path is configured; no-op for memory-only.
    pub fn save(&self) -> Result<(), JerryError> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| JerryError::Storage(format!("mkdir: {}", err.kind())))?;
        }
        let file = CredentialsFile {
            schema_version: CREDENTIALS_SCHEMA_VERSION,
            providers: self.providers.clone(),
            selected: self.selected.clone(),
        };
        let json = serde_json::to_string_pretty(&file)
            .map_err(|err| JerryError::Storage(format!("serialize credentials: {err}")))?;
        std::fs::write(path, json)
            .map_err(|err| JerryError::Storage(format!("write credentials: {}", err.kind())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_api_key() {
        let cred = ProviderCredential::new("sk-super-secret-value-123456");
        let dbg = format!("{cred:?}");
        assert!(!dbg.contains("sk-super-secret"));
        assert!(dbg.contains("REDACTED"));
    }

    #[test]
    fn empty_key_rejected() {
        let mut store = CredentialStore::memory_only();
        let err = store
            .put(ProviderKind::OpenAi, ProviderCredential::new("   "))
            .unwrap_err();
        assert!(matches!(err, JerryError::InvalidRequest(_)));
    }

    #[test]
    fn memory_store_round_trip() {
        let mut store = CredentialStore::memory_only();
        store
            .put(
                ProviderKind::Anthropic,
                ProviderCredential::new("sk-ant-test"),
            )
            .unwrap();
        assert!(store.has_key(ProviderKind::Anthropic));
        assert_eq!(store.selected(), Some("anthropic"));
        assert!(store
            .key_hint(ProviderKind::Anthropic)
            .unwrap()
            .ends_with("test"));
    }

    #[test]
    fn file_round_trip_and_redacted_debug_on_store() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("halley-jerry-creds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(CREDENTIALS_FILE_NAME);

        {
            let mut store = CredentialStore::at_path(&path).unwrap();
            store
                .put(
                    ProviderKind::Groq,
                    ProviderCredential::new("gsk_super_secret_key_value"),
                )
                .unwrap();
        }

        let loaded = CredentialStore::at_path(&path).unwrap();
        assert!(loaded.has_key(ProviderKind::Groq));
        // Store Debug must not leak the key either.
        let dbg = format!("{loaded:?}");
        assert!(!dbg.contains("gsk_super_secret_key_value"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn schema_mismatch_rejected() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("halley-jerry-creds-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(CREDENTIALS_FILE_NAME);
        std::fs::write(
            &path,
            r#"{"schema_version":99,"providers":{},"selected":null}"#,
        )
        .unwrap();
        let err = CredentialStore::at_path(&path).unwrap_err();
        assert!(err.to_string().contains("schema_version"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
