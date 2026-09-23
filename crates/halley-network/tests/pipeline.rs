//! Network pipeline integration tests (no sockets — mock transport only).

use halley_common::Config;
use halley_network::{
    HttpMethod, MockTransport, NetworkError, NetworkManager, NetworkPolicy, NetworkRequest,
    RequestContext, REDACTED,
};
use url::Url;

#[test]
fn default_manager_performs_no_io() {
    let mut manager = NetworkManager::new(&Config::default());
    let req = NetworkRequest::new(HttpMethod::Get, "https://example.com/").unwrap();
    let ctx = RequestContext::document("https://example.com/", false);
    assert_eq!(manager.execute(&req, &ctx), Err(NetworkError::NoTransport));
}

#[test]
fn policy_allows_https_and_blocks_unknown_via_request_constructor() {
    let policy = NetworkPolicy::default();
    let ctx = RequestContext::default();
    let req = NetworkRequest::new(HttpMethod::Get, "https://example.com/").unwrap();
    assert_eq!(
        policy.evaluate(&req, &ctx),
        halley_network::RequestDecision::Allow
    );
    assert!(NetworkRequest::new(HttpMethod::Get, "javascript:alert(1)").is_err());
}

#[test]
fn redirect_limit_returns_typed_error() {
    let mut config = Config::default();
    config.network.max_redirects = 1;
    let transport = MockTransport::new()
        .push_redirect(302, "https://a.example/1", "https://a.example/2")
        .push_redirect(302, "https://a.example/2", "https://a.example/3");
    let mut manager = NetworkManager::with_transport(&config, Box::new(transport));
    let req = NetworkRequest::new(HttpMethod::Get, "https://a.example/1").unwrap();
    let err = manager
        .execute(&req, &RequestContext::default())
        .unwrap_err();
    assert_eq!(err, NetworkError::RedirectLimitExceeded);
}

#[test]
fn timeouts_are_configurable_and_centralized() {
    let mut config = Config::default();
    config.network.request_timeout_ms = 45_000;
    let manager = NetworkManager::new(&config);
    assert_eq!(manager.timeouts().request().as_millis(), 45_000);
}

#[test]
fn secret_headers_never_appear_in_redacted_log_helpers() {
    let headers = vec![
        ("Authorization".into(), "Bearer SUPER_SECRET".into()),
        ("Cookie".into(), "sid=SUPER_SECRET".into()),
    ];
    let text = halley_network::redact_headers_for_log(&headers);
    assert!(!text.contains("SUPER_SECRET"));
    assert!(text.contains(REDACTED));
}

#[test]
fn tls_verification_default_is_on() {
    let manager = NetworkManager::new(&Config::default());
    assert!(manager.tls_policy().verifies_certificates());
    assert!(manager.tls_policy().is_release_safe());
}

#[test]
fn context_classifies_private_mode_flag() {
    let ctx = RequestContext::document("https://example.com/", true);
    assert!(ctx.private_mode);
    let url = Url::parse("https://example.com/").unwrap();
    assert_eq!(url.host_str(), Some("example.com"));
}
