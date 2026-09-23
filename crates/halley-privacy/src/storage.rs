//! Storage layout, path safety, and the centralized storage manager.
//!
//! Categories are **never mixed**: profile config, cache, cookies, session,
//! logs, and temp live in sibling directories under a profile root.
//! Only [`StorageManager`] constructs paths under that root — other crates
//! must not join path segments from URLs, titles, or page-controlled data
//! without [`sanitize_component`] / [`safe_join_component`].

use std::path::{Path, PathBuf};

use crate::error::StorageError;

/// Storage category names under a profile root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoragePaths {
    /// General profile data (config, future history, etc.).
    Profile,
    /// HTTP/page cache (normal profile only when enabled).
    Cache,
    /// Persistent cookie store files (normal profile).
    Cookies,
    /// Session snapshots / runtime session files.
    Session,
    /// Local logs (never remote).
    Logs,
    /// Temporary / private-session scratch space.
    Temp,
}

impl StoragePaths {
    /// Directory name under the profile root.
    pub fn dir_name(self) -> &'static str {
        match self {
            StoragePaths::Profile => "profile",
            StoragePaths::Cache => "cache",
            StoragePaths::Cookies => "cookies",
            StoragePaths::Session => "session",
            StoragePaths::Logs => "logs",
            StoragePaths::Temp => "temp",
        }
    }

    /// Parse a category name (for diagnostics / dynamic clear ops).
    pub fn parse(name: &str) -> Result<StoragePaths, StorageError> {
        match name {
            "profile" => Ok(StoragePaths::Profile),
            "cache" => Ok(StoragePaths::Cache),
            "cookies" => Ok(StoragePaths::Cookies),
            "session" => Ok(StoragePaths::Session),
            "logs" => Ok(StoragePaths::Logs),
            "temp" => Ok(StoragePaths::Temp),
            other => Err(StorageError::UnknownCategory(other.to_string())),
        }
    }
}

/// Validate a single path component derived from untrusted input.
///
/// Rejects empty, `.`, `..`, separators, drive prefixes, NUL, and
/// percent-encoded traversal sequences (`%2e%2e`, `%2f`, `%5c`).
pub fn sanitize_component(raw: &str) -> Result<String, StorageError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err(StorageError::UnsafeComponent);
    }
    let lower = trimmed.to_ascii_lowercase();
    // Percent-encoded traversal / separators.
    for pattern in ["%2e", "%2f", "%5c", "%00", "%2e%2e", "..\\", "../", ".."] {
        if lower.contains(pattern) {
            return Err(StorageError::UnsafeComponent);
        }
    }
    if trimmed.contains(['/', '\\', '\0', ':']) {
        return Err(StorageError::UnsafeComponent);
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(StorageError::UnsafeComponent);
    }
    Ok(trimmed.to_string())
}

/// Join `component` under `root`, ensuring the result stays inside `root`.
pub fn safe_join_component(root: &Path, component: &str) -> Result<PathBuf, StorageError> {
    let safe = sanitize_component(component)?;
    let candidate = root.join(&safe);
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    // Candidate may not exist yet — canonicalize parent if needed.
    let candidate_canon = candidate.canonicalize().unwrap_or_else(|_| {
        candidate
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .map(|p| p.join(safe.as_str()))
            .unwrap_or(candidate.clone())
    });
    if !candidate_canon.starts_with(&root_canon) {
        return Err(StorageError::PathEscapesRoot);
    }
    Ok(candidate)
}

/// Filesystem-backed storage manager for one profile root.
#[derive(Debug, Clone)]
pub struct StorageManager {
    root: PathBuf,
}

impl StorageManager {
    /// Use an existing directory as the profile root (must exist or be creatable).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        StorageManager { root: root.into() }
    }

    /// Profile root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Absolute-ish path for a storage category directory.
    pub fn category_path(&self, category: StoragePaths) -> PathBuf {
        self.root.join(category.dir_name())
    }

    /// Create the standard directory tree (idempotent).
    pub fn ensure_directories(&self) -> Result<(), StorageError> {
        for category in [
            StoragePaths::Profile,
            StoragePaths::Cache,
            StoragePaths::Cookies,
            StoragePaths::Session,
            StoragePaths::Logs,
            StoragePaths::Temp,
        ] {
            std::fs::create_dir_all(self.category_path(category))?;
        }
        Ok(())
    }

    /// Safe path for an untrusted name under a category (e.g. site id).
    pub fn safe_path(
        &self,
        category: StoragePaths,
        component: &str,
    ) -> Result<PathBuf, StorageError> {
        let base = self.category_path(category);
        std::fs::create_dir_all(&base).map_err(StorageError::from)?;
        safe_join_component(&base, component)
    }

    /// Remove everything under `temp` (private cleanup / startup sweep).
    pub fn clear_temp(&self) -> Result<(), StorageError> {
        let temp = self.category_path(StoragePaths::Temp);
        if temp.exists() {
            std::fs::remove_dir_all(&temp)?;
        }
        std::fs::create_dir_all(&temp)?;
        Ok(())
    }

    /// Remove the cache directory tree (user-initiated clear cache).
    pub fn clear_cache(&self) -> Result<(), StorageError> {
        let cache = self.category_path(StoragePaths::Cache);
        if cache.exists() {
            std::fs::remove_dir_all(&cache)?;
        }
        std::fs::create_dir_all(&cache)?;
        Ok(())
    }

    /// Remove cookies directory (persistent jar files — not engine jar).
    pub fn clear_cookies_dir(&self) -> Result<(), StorageError> {
        let cookies = self.category_path(StoragePaths::Cookies);
        if cookies.exists() {
            std::fs::remove_dir_all(&cookies)?;
        }
        std::fs::create_dir_all(&cookies)?;
        Ok(())
    }

    /// Site-scoped clear placeholder: only removes a safe component path
    /// under one category — never wipes the whole root on empty input.
    pub fn clear_site_component(
        &self,
        category: StoragePaths,
        component: &str,
    ) -> Result<bool, StorageError> {
        // Empty / malformed component must not mean "clear everything".
        let path = self.safe_path(category, component)?;
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
            return Ok(true);
        }
        Ok(false)
    }
}

/// Platform default user-data root for Halley profiles (no telemetry paths).
pub fn default_profile_root(app_name: &str) -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support"))
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."))
    };
    base.join(app_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("halley-storage-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sanitize_rejects_traversal_patterns() {
        for bad in [
            "..",
            "../",
            "..\\",
            "../../",
            "%2e%2e",
            "%2E%2E",
            "a/b",
            "a\\b",
            "",
            ".",
            "foo:bar",
            "nul\0byte",
        ] {
            assert!(
                sanitize_component(bad).is_err(),
                "expected reject for {bad:?}"
            );
        }
        assert_eq!(sanitize_component(" example.com ").unwrap(), "example.com");
    }

    #[test]
    fn ensure_directories_creates_all_categories() {
        let root = temp_root("dirs");
        let mgr = StorageManager::new(&root);
        mgr.ensure_directories().unwrap();
        for cat in ["profile", "cache", "cookies", "session", "logs", "temp"] {
            assert!(root.join(cat).is_dir(), "missing {cat}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn safe_path_rejects_escape_attempts() {
        let root = temp_root("safe");
        let mgr = StorageManager::new(&root);
        mgr.ensure_directories().unwrap();
        assert!(mgr.safe_path(StoragePaths::Cache, "..").is_err());
        assert!(mgr.safe_path(StoragePaths::Cache, "../cookies").is_err());
        assert!(mgr.safe_path(StoragePaths::Cookies, "%2e%2e").is_err());
        let ok = mgr.safe_path(StoragePaths::Cache, "example.com").unwrap();
        assert!(ok.starts_with(root.join("cache")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn categories_are_separate_directories() {
        let root = temp_root("split");
        let mgr = StorageManager::new(&root);
        mgr.ensure_directories().unwrap();
        assert_ne!(
            mgr.category_path(StoragePaths::Cache),
            mgr.category_path(StoragePaths::Cookies)
        );
        assert_ne!(
            mgr.category_path(StoragePaths::Session),
            mgr.category_path(StoragePaths::Logs)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clear_temp_only_removes_temp() {
        let root = temp_root("temp");
        let mgr = StorageManager::new(&root);
        mgr.ensure_directories().unwrap();
        std::fs::write(mgr.category_path(StoragePaths::Profile).join("keep"), b"x").unwrap();
        std::fs::write(mgr.category_path(StoragePaths::Temp).join("scratch"), b"y").unwrap();
        mgr.clear_temp().unwrap();
        assert!(mgr
            .category_path(StoragePaths::Profile)
            .join("keep")
            .exists());
        assert!(!mgr
            .category_path(StoragePaths::Temp)
            .join("scratch")
            .exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clear_site_with_empty_or_bad_component_never_wipes_root() {
        let root = temp_root("site");
        let mgr = StorageManager::new(&root);
        mgr.ensure_directories().unwrap();
        std::fs::write(root.join("profile/important"), b"data").unwrap();
        assert!(mgr.clear_site_component(StoragePaths::Cache, "..").is_err());
        assert!(mgr.clear_site_component(StoragePaths::Cache, "").is_err());
        assert!(root.join("profile/important").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn default_profile_root_is_under_platform_dir() {
        let root = default_profile_root("Halley");
        assert!(root.ends_with("Halley"));
    }
}
