//! Deterministic property-style tests: path safety and cookie isolation.

/// Simple LCG for deterministic pseudo-random strings (no extra deps).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn string(&mut self, len: usize) -> String {
        const ALPHAB: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789._-%\\/";
        (0..len)
            .map(|_| {
                let i = (self.next() % ALPHAB.len() as u64) as usize;
                ALPHAB[i] as char
            })
            .collect()
    }
}

#[test]
fn random_untrusted_components_never_escape_cache_root() {
    let root = std::env::temp_dir().join(format!("halley-prop-path-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mgr = halley_privacy::StorageManager::new(&root);
    mgr.ensure_directories().unwrap();
    let cache_root = mgr.category_path(halley_privacy::storage::StoragePaths::Cache);

    let mut rng = Lcg(0xA11E7 ^ u64::from(std::process::id()));
    let _ = rng.next();

    for _ in 0..500 {
        let n = rng.next();
        let candidate = rng.string(1 + (n % 24) as usize);
        if let Ok(path) = mgr.safe_path(halley_privacy::storage::StoragePaths::Cache, &candidate) {
            assert!(
                path.starts_with(&cache_root),
                "escaped for {candidate:?} -> {path:?}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cookie_domain_matching_does_not_leak_across_unrelated_sites() {
    use halley_privacy::{CookiePolicy, CookieStore, SiteContext};

    let mut store = CookieStore::new(false, CookiePolicy::default());
    let site = SiteContext::for_url("https://victim.example/", false).unwrap();
    store
        .set_from_header("sid=SECRETVICTIM; Path=/", "https://victim.example/", &site)
        .unwrap();

    let mut rng = Lcg(0xC0_0C_1E);
    for _ in 0..200 {
        let n = rng.next();
        let label = rng.string(1 + (n % 8) as usize);
        let host = format!("{label}.example");
        if host.contains("victim.example") {
            continue;
        }
        let url = format!("https://{}/", host.replace(['/', '\\'], "x"));
        let Ok(u) = url::Url::parse(&url) else {
            continue;
        };
        let Ok(origin) = halley_common::Origin::from_url(&u) else {
            continue;
        };
        if origin.host() == "victim.example" {
            continue;
        }
        let Ok(site_x) = SiteContext::for_url(&format!("https://{}", origin.host()), false) else {
            continue;
        };
        let cookies = store.cookies_for_request(&url, &site_x);
        for (_, value) in cookies {
            assert_ne!(value, "SECRETVICTIM", "leaked to {url}");
        }
    }
}
