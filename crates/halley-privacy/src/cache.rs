//! Cache policy (normal vs private).

/// How HTTP/page cache may be used for a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachePolicy {
    /// Normal profile may write a persistent cache directory.
    pub persistent_cache_enabled: bool,
    /// Private sessions may use an ephemeral cache for the session only.
    pub private_ephemeral_cache: bool,
}

impl CachePolicy {
    /// Defaults from `Config::default().storage.cache_enabled`.
    pub fn from_config(cache_enabled: bool) -> Self {
        CachePolicy {
            persistent_cache_enabled: cache_enabled,
            private_ephemeral_cache: true,
        }
    }

    /// Whether cache writes are allowed for this profile kind.
    pub fn allows_write(&self, private: bool) -> bool {
        if private {
            self.private_ephemeral_cache
        } else {
            self.persistent_cache_enabled
        }
    }

    /// Whether cache data may be retained after the session ends.
    pub fn allows_persist_after_close(&self, private: bool) -> bool {
        !private && self.persistent_cache_enabled
    }
}

impl Default for CachePolicy {
    fn default() -> Self {
        CachePolicy {
            persistent_cache_enabled: true,
            private_ephemeral_cache: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_cache_is_ephemeral() {
        let policy = CachePolicy::default();
        assert!(policy.allows_write(true));
        assert!(!policy.allows_persist_after_close(true));
        assert!(policy.allows_persist_after_close(false));
    }

    #[test]
    fn disabled_normal_cache_blocks_writes() {
        let policy = CachePolicy::from_config(false);
        assert!(!policy.allows_write(false));
        assert!(policy.allows_write(true)); // private still ephemeral-capable
    }
}
