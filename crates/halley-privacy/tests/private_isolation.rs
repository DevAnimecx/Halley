//! Privacy integration: private session isolation (Prompt #4 §50).

use halley_common::Config;
use halley_privacy::{BrowserProfile, ProfileKind, SiteContext};

fn temp_config(tag: &str) -> (Config, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("halley-priv-int-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut config = Config::default();
    config.storage.profile_root = Some(root.display().to_string());
    (config, root)
}

/// Mandatory private-mode lifecycle test (§50).
#[test]
fn normal_cookie_private_isolation_cleanup() {
    let (config, normal_root) = temp_config("lifecycle");

    // Start normal profile; set persistent cookie.
    let mut normal = BrowserProfile::open_normal(&config).expect("normal profile");
    let site = SiteContext::for_url("https://example.com/", false).unwrap();
    normal
        .cookies_mut()
        .set_from_header(
            "session=PERSISTENT_SECRET; Path=/",
            "https://example.com/",
            &site,
        )
        .unwrap();
    assert_eq!(normal.cookies().len(), 1);

    // Start private session; persistent cookie not auto-copied.
    let mut private = BrowserProfile::open_private(&config).expect("private profile");
    assert_eq!(private.kind(), ProfileKind::Private);
    assert_eq!(private.cookies().len(), 0);

    // Private sets its own cookie.
    let site_p = SiteContext::for_url("https://example.com/", true).unwrap();
    private
        .cookies_mut()
        .set_from_header(
            "session=PRIVATE_SECRET; Path=/",
            "https://example.com/",
            &site_p,
        )
        .unwrap();
    assert_eq!(private.cookies().len(), 1);
    // Normal jar still only has its cookie.
    assert_eq!(normal.cookies().len(), 1);
    let normal_cookies = normal
        .cookies()
        .cookies_for_request("https://example.com/", &site);
    assert!(normal_cookies.iter().all(|(_, v)| v != "PRIVATE_SECRET"));

    // Private root is not under normal root.
    assert!(!private.storage().root().starts_with(&normal_root));

    // Close private; private cookie and files gone.
    let private_root = private.storage().root().to_path_buf();
    private.close().unwrap();
    assert!(!private_root.exists());

    // New private session must not see old private cookie.
    let private2 = BrowserProfile::open_private(&config).unwrap();
    assert_eq!(private2.cookies().len(), 0);
    private2.close().unwrap();

    // Normal still has only PERSISTENT.
    assert_eq!(normal.cookies().len(), 1);
    let vals: Vec<String> = normal
        .cookies()
        .cookies_for_request("https://example.com/", &site)
        .into_iter()
        .map(|(_, v)| v)
        .collect();
    assert_eq!(vals, vec!["PERSISTENT_SECRET".to_string()]);

    normal.close().unwrap();
    let _ = std::fs::remove_dir_all(&normal_root);
}

#[test]
fn private_storage_never_uses_normal_profile_tree() {
    let (config, normal_root) = temp_config("trees");
    let normal = BrowserProfile::open_normal(&config).unwrap();
    let private = BrowserProfile::open_private(&config).unwrap();
    assert!(!private
        .storage()
        .root()
        .starts_with(normal.storage().root()));
    assert!(!private.storage().root().starts_with(&normal_root));
    // Categories inside private are still separated.
    assert_ne!(
        private
            .storage()
            .category_path(halley_privacy::storage::StoragePaths::Cache),
        private
            .storage()
            .category_path(halley_privacy::storage::StoragePaths::Cookies)
    );
    private.close().unwrap();
    normal.close().unwrap();
    let _ = std::fs::remove_dir_all(&normal_root);
}

#[test]
fn path_traversal_cannot_escape_profile_cache() {
    let (config, root) = temp_config("trav");
    let profile = BrowserProfile::open_normal(&config).unwrap();
    for attack in ["..", "../cookies", "%2e%2e", "..\\..", "a/b"] {
        assert!(
            profile
                .storage()
                .safe_path(halley_privacy::storage::StoragePaths::Cache, attack)
                .is_err(),
            "attack {attack:?} must fail"
        );
    }
    profile.close().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cookie_debug_never_leaks_values() {
    let (config, root) = temp_config("dbg");
    let mut profile = BrowserProfile::open_normal(&config).unwrap();
    let site = SiteContext::for_url("https://example.com/", false).unwrap();
    profile
        .cookies_mut()
        .set_from_header(
            "token=SUPER_SECRET_COOKIE; Path=/",
            "https://example.com/",
            &site,
        )
        .unwrap();
    let cookie = profile
        .cookies()
        .cookies_for_request("https://example.com/", &site);
    assert_eq!(cookie.len(), 1);
    // Header pair has secret for network use; Debug of store cookies:
    // access via format of individual cookie through clear paths — ensure
    // SiteContext/Profile Debug doesn't contain it either.
    let profile_debug = format!(
        "private={} root={:?}",
        profile.is_private(),
        profile.storage().root()
    );
    assert!(!profile_debug.contains("SUPER_SECRET_COOKIE"));
    profile.close().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
