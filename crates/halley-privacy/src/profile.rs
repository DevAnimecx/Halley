//! Browser profiles: normal (persistent) vs private (ephemeral).
//!
//! A profile owns persistent storage boundaries, its privacy policy, and
//! its cookie jar. Tabs, pages, and Jerry sessions are **not** profiles —
//! they hang off a profile at runtime (ADR-009).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use halley_common::Config;

use crate::cookie::{CookiePolicy, CookieStore};
use crate::error::ProfileError;
use crate::policy::PrivacyPolicy;
use crate::storage::{default_profile_root, StorageManager};

/// Profile kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    /// Persistent user profile (cookies/cache/session on disk under root).
    Normal,
    /// Private browsing: isolated temp tree, destroyed on close.
    Private,
}

/// Stable profile id for diagnostics (not a filesystem secret).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileId(String);

impl ProfileId {
    /// Display string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

static PROFILE_SEQ: AtomicU64 = AtomicU64::new(1);

/// An open browser profile: storage + privacy + cookies.
pub struct BrowserProfile {
    id: ProfileId,
    kind: ProfileKind,
    storage: StorageManager,
    privacy: PrivacyPolicy,
    cookies: CookieStore,
    /// Temp root to delete on private close (`None` for normal).
    private_temp_root: Option<PathBuf>,
}

impl BrowserProfile {
    /// Open (or create) the normal profile under the configured root.
    ///
    /// Directories are created eagerly so startup fails loudly if the disk
    /// is unavailable rather than mid-navigation.
    pub fn open_normal(config: &Config) -> Result<Self, ProfileError> {
        let root = config
            .storage
            .profile_root
            .clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| default_profile_root(&config.app.app_name));
        Self::open_at(ProfileKind::Normal, root, config, None)
    }

    /// Open a private profile in a fresh directory under the system temp dir.
    ///
    /// Never reuses the normal profile root.
    pub fn open_private(config: &Config) -> Result<Self, ProfileError> {
        let seq = PROFILE_SEQ.fetch_add(1, Ordering::Relaxed);
        let mut base = std::env::temp_dir();
        base.push(format!("halley-private-{}-{}", std::process::id(), seq));
        Self::open_at(ProfileKind::Private, base.clone(), config, Some(base))
    }

    fn open_at(
        kind: ProfileKind,
        root: PathBuf,
        config: &Config,
        private_temp_root: Option<PathBuf>,
    ) -> Result<Self, ProfileError> {
        // Guard: private must never sit inside a configured normal root.
        if kind == ProfileKind::Private {
            if let Some(normal) = config.storage.profile_root.as_ref().map(PathBuf::from) {
                if root.starts_with(&normal) {
                    return Err(ProfileError::NotAllowed);
                }
            }
        }
        let storage = StorageManager::new(root);
        storage
            .ensure_directories()
            .map_err(|_| ProfileError::CreateFailed)?;
        if kind == ProfileKind::Private {
            storage
                .clear_temp()
                .map_err(|_| ProfileError::CreateFailed)?;
        }
        let privacy = PrivacyPolicy::from_config(config);
        let private = matches!(kind, ProfileKind::Private);
        let cookies = CookieStore::new(private, privacy.cookies.clone());
        let seq = PROFILE_SEQ.fetch_add(1, Ordering::Relaxed);
        let prefix = match kind {
            ProfileKind::Normal => "normal",
            ProfileKind::Private => "private",
        };
        Ok(BrowserProfile {
            id: ProfileId(format!("{prefix}-{seq}")),
            kind,
            storage,
            privacy,
            cookies,
            private_temp_root,
        })
    }

    /// Profile id.
    pub fn id(&self) -> &ProfileId {
        &self.id
    }

    /// Normal vs private.
    pub fn kind(&self) -> ProfileKind {
        self.kind
    }

    /// Whether this is a private (ephemeral) profile.
    pub fn is_private(&self) -> bool {
        matches!(self.kind, ProfileKind::Private)
    }

    /// Storage manager for this profile's directories.
    pub fn storage(&self) -> &StorageManager {
        &self.storage
    }

    /// Privacy policy aggregate.
    pub fn privacy(&self) -> &PrivacyPolicy {
        &self.privacy
    }

    /// Cookie jar for this profile (isolated for private).
    pub fn cookies(&self) -> &CookieStore {
        &self.cookies
    }

    /// Mutable cookie jar (adapter / clear-site-data).
    pub fn cookies_mut(&mut self) -> &mut CookieStore {
        &mut self.cookies
    }

    /// Cookie policy helper for this profile.
    pub fn cookie_policy(&self) -> &CookiePolicy {
        &self.privacy.cookies
    }

    /// Close the profile.
    ///
    /// Private: deletes the entire private temp root (cookies, cache,
    /// session scratch). Normal: purges expired cookies in memory only —
    /// persistent directories remain for the next run.
    pub fn close(self) -> Result<(), ProfileError> {
        match self.kind {
            ProfileKind::Normal => Ok(()),
            ProfileKind::Private => {
                let root = self
                    .private_temp_root
                    .as_ref()
                    .ok_or(ProfileError::CleanupFailed)?;
                if root.exists() {
                    std::fs::remove_dir_all(root).map_err(|_| ProfileError::CleanupFailed)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_root(root: &std::path::Path) -> Config {
        let mut config = Config::default();
        config.storage.profile_root = Some(root.to_string_lossy().into_owned());
        config
    }

    #[test]
    fn normal_profile_creates_separated_categories() {
        let root = std::env::temp_dir().join(format!("halley-normal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let config = config_with_root(&root);
        let profile = BrowserProfile::open_normal(&config).unwrap();
        assert!(!profile.is_private());
        assert!(profile
            .storage()
            .category_path(crate::storage::StoragePaths::Cookies)
            .is_dir());
        assert!(profile
            .storage()
            .category_path(crate::storage::StoragePaths::Cache)
            .is_dir());
        profile.close().unwrap();
        // Normal close keeps directories.
        assert!(root.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn private_profile_is_not_under_normal_root_and_cleans_up() {
        let normal_root = std::env::temp_dir().join(format!("halley-nrm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&normal_root);
        let config = config_with_root(&normal_root);
        let private = BrowserProfile::open_private(&config).unwrap();
        assert!(private.is_private());
        let private_path = private.storage().root().to_path_buf();
        assert!(!private_path.starts_with(&normal_root));
        // Write scratch data into private temp.
        std::fs::write(
            private
                .storage()
                .category_path(crate::storage::StoragePaths::Temp)
                .join("x"),
            b"secret",
        )
        .unwrap();
        private.close().unwrap();
        assert!(!private_path.exists(), "private root must be deleted");
        let _ = std::fs::remove_dir_all(&normal_root);
    }

    #[test]
    fn private_jars_are_isolated_from_normal_in_memory() {
        let normal_root = std::env::temp_dir().join(format!("halley-iso-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&normal_root);
        let config = config_with_root(&normal_root);
        let mut normal = BrowserProfile::open_normal(&config).unwrap();
        let mut private = BrowserProfile::open_private(&config).unwrap();

        let site = crate::site::SiteContext::for_url("https://example.com/", false).unwrap();
        normal
            .cookies_mut()
            .set_from_header("sid=persist; Path=/", "https://example.com/", &site)
            .unwrap();
        assert_eq!(normal.cookies().len(), 1);
        assert_eq!(private.cookies().len(), 0);

        let site_p = crate::site::SiteContext::for_url("https://example.com/", true).unwrap();
        private
            .cookies_mut()
            .set_from_header("sid=ephemeral; Path=/", "https://example.com/", &site_p)
            .unwrap();
        assert_eq!(private.cookies().len(), 1);
        assert_eq!(normal.cookies().len(), 1);

        private.close().unwrap();
        // Normal jar unchanged after private close.
        assert_eq!(normal.cookies().len(), 1);
        normal.close().unwrap();
        let _ = std::fs::remove_dir_all(&normal_root);
    }
}
