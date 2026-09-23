//! Prompt #5 regression: Jerry host + chrome panel wiring with MockEngine
//! and MockTransport (no network I/O).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use halley_common::Config;
use halley_core::{apply_jerry_events, Browser, BrowserEvent, JerryHost, CONVERSATIONS_FILE_NAME};
use halley_engine::{ChromeAction, ChromeState, MockEngine};
use halley_jerry::{MockTransport, ProviderTransport};

fn browser_with_mock_engine(config: Config) -> Browser<MockEngine> {
    Browser::new(MockEngine::new(), config, "about:blank").unwrap()
}

fn memory_host(config: &Config, transport: Arc<dyn ProviderTransport>) -> JerryHost {
    JerryHost::memory_only(config, transport, None)
}

fn wait_until(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if pred() {
            return true;
        }
        if Instant::now() >= deadline {
            return pred();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn jerry_chrome_action_routes_to_browser_event() {
    let config = Config::default();
    let mut browser = browser_with_mock_engine(config);
    browser
        .engine_mut()
        .inject_chrome_action(ChromeAction::JerryToggle);
    browser.process_messages().unwrap();
    let drained = browser.drain_events();
    assert!(
        drained
            .iter()
            .any(|e| matches!(e, BrowserEvent::JerryAction(ChromeAction::JerryToggle))),
        "expected JerryToggle event, got {drained:?}"
    );

    browser
        .engine_mut()
        .inject_chrome_action(ChromeAction::JerrySend("hi".into()));
    browser.process_messages().unwrap();
    let drained = browser.drain_events();
    assert!(drained
        .iter()
        .any(|e| matches!(e, BrowserEvent::JerryAction(ChromeAction::JerrySend(_)))));
}

#[test]
fn panel_open_expands_chrome_height() {
    let config = Config::default();
    let mut browser = browser_with_mock_engine(config.clone());
    let base = browser.chrome_height();
    assert_eq!(base, config.window.chrome_height);

    let mut jerry = browser.jerry_state().clone();
    jerry.open = true;
    browser.set_jerry_state(jerry);
    assert_eq!(
        browser.chrome_height(),
        config.window.chrome_height + config.jerry.panel_height
    );

    let mut closed = browser.jerry_state().clone();
    closed.open = false;
    browser.set_jerry_state(closed);
    assert_eq!(browser.chrome_height(), config.window.chrome_height);
}

#[test]
fn chrome_snapshot_never_contains_api_key_material() {
    let config = Config::default();
    let mut browser = browser_with_mock_engine(config.clone());
    let transport: Arc<dyn ProviderTransport> = Arc::new(MockTransport::new());
    let mut host = memory_host(&config, transport);
    host.handle_action(ChromeAction::JerryEnable, &browser)
        .unwrap();
    host.handle_action(ChromeAction::JerrySetProvider("openai".into()), &browser)
        .unwrap();
    host.handle_action(
        ChromeAction::JerrySetKey("sk-live-secret-abcdef123456".into()),
        &browser,
    )
    .unwrap();

    let state = host.snapshot();
    assert!(state.has_api_key);
    assert!(!state.provider.is_empty());
    browser.set_jerry_state(state.clone());

    let snapshot = browser.chrome_snapshot();
    let json = format!("{snapshot:?}");
    assert!(
        !json.contains("sk-live-secret"),
        "key leaked into chrome state debug: {json}"
    );
    let jerry_debug = format!("{state:?}");
    assert!(
        !jerry_debug.contains("sk-live-secret"),
        "key leaked into JerryChromeState debug: {jerry_debug}"
    );
    let _ = ChromeState::default();
}

#[test]
fn mock_transport_chat_flow_completes_without_network() {
    let config = Config::default();
    let browser = browser_with_mock_engine(config.clone());
    let transport = Arc::new(MockTransport::new());
    let shared: Arc<dyn ProviderTransport> = transport.clone();
    let mut host = memory_host(&config, shared);

    host.handle_action(ChromeAction::JerryEnable, &browser)
        .unwrap();
    host.handle_action(ChromeAction::JerrySetProvider("openai".into()), &browser)
        .unwrap();
    host.handle_action(
        ChromeAction::JerrySetKey("sk-test-key-abcdef123456".into()),
        &browser,
    )
    .unwrap();

    host.handle_action(ChromeAction::JerrySend("hello jerry".into()), &browser)
        .unwrap();
    // Status starts as "connecting"; becomes "streaming" once partial text
    // arrives in poll(); snapshot no longer force-overrides to "streaming".
    let early = host.snapshot().status;
    assert!(
        early == "connecting" || early == "streaming",
        "expected connecting/streaming, got {early}"
    );

    let done = wait_until(Duration::from_secs(5), || {
        host.poll();
        !host.is_busy()
    });
    assert!(done, "completion did not finish in time");

    let snap = host.snapshot();
    assert!(
        snap.messages.iter().any(|m| m.role == "assistant"),
        "expected an assistant message: {:?}",
        snap.messages
    );
    assert_eq!(transport.call_count(), 1);
    assert!(snap.error.is_none(), "unexpected error: {:?}", snap.error);
    assert_eq!(snap.status, "idle");
}

#[test]
fn apply_jerry_events_pushes_actions_into_host() {
    let config = Config::default();
    let browser = browser_with_mock_engine(config.clone());
    let transport: Arc<dyn ProviderTransport> = Arc::new(MockTransport::new());
    let mut host = memory_host(&config, transport);

    let events = vec![BrowserEvent::JerryAction(ChromeAction::JerryToggle)];
    apply_jerry_events(&mut host, &browser, &events);
    assert!(host.snapshot().open, "toggle should open the panel");
}

#[test]
fn conversations_file_name_is_stable() {
    assert_eq!(CONVERSATIONS_FILE_NAME, "jerry-conversations.json");
}

#[test]
fn waker_is_invoked_on_job_completion() {
    let config = Config::default();
    let browser = browser_with_mock_engine(config.clone());
    let woke = Arc::new(AtomicBool::new(false));
    let flag = woke.clone();
    let waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        flag.store(true, Ordering::SeqCst);
    });
    let transport: Arc<dyn ProviderTransport> = Arc::new(MockTransport::new());
    let mut host = JerryHost::memory_only(&config, transport, Some(waker));

    host.handle_action(ChromeAction::JerryEnable, &browser)
        .unwrap();
    host.handle_action(ChromeAction::JerrySetProvider("openai".into()), &browser)
        .unwrap();
    host.handle_action(
        ChromeAction::JerrySetKey("sk-test-key-abcdef123456".into()),
        &browser,
    )
    .unwrap();
    host.handle_action(ChromeAction::JerrySend("ping".into()), &browser)
        .unwrap();

    let woke_ok = wait_until(Duration::from_secs(5), || woke.load(Ordering::SeqCst));
    assert!(woke_ok, "worker should wake the UI loop after completion");
    let _ = wait_until(Duration::from_secs(2), || {
        host.poll();
        !host.is_busy()
    });
}

#[test]
fn send_without_key_surfaces_error_not_fake_success() {
    let config = Config::default();
    let browser = browser_with_mock_engine(config.clone());
    let transport: Arc<dyn ProviderTransport> = Arc::new(MockTransport::new());
    let mut host = memory_host(&config, transport);
    host.handle_action(ChromeAction::JerryEnable, &browser)
        .unwrap();
    host.handle_action(ChromeAction::JerrySetProvider("openai".into()), &browser)
        .unwrap();

    let err = host
        .handle_action(ChromeAction::JerrySend("hi".into()), &browser)
        .unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("key") || msg.contains("credential") || msg.contains("missing"),
        "expected missing-key style error, got: {msg}"
    );
    assert!(!host.is_busy());
    assert_ne!(host.snapshot().status, "streaming");
    assert_ne!(host.snapshot().status, "connecting");
}

#[test]
fn apply_jerry_events_does_not_abort_batch_on_failure() {
    let config = Config::default();
    let browser = browser_with_mock_engine(config.clone());
    let transport: Arc<dyn ProviderTransport> = Arc::new(MockTransport::new());
    let mut host = memory_host(&config, transport);
    host.handle_action(ChromeAction::JerryEnable, &browser)
        .unwrap();
    host.handle_action(ChromeAction::JerrySetProvider("openai".into()), &browser)
        .unwrap();

    let events = vec![
        // Fails: no API key yet.
        BrowserEvent::JerryAction(ChromeAction::JerrySend("hi".into())),
        // Must still run after the failure.
        BrowserEvent::JerryAction(ChromeAction::JerryToggle),
    ];
    apply_jerry_events(&mut host, &browser, &events);

    let snap = host.snapshot();
    assert!(snap.error.is_some(), "failed action should surface error");
    assert!(snap.open, "later event in the same batch must still apply");
}
