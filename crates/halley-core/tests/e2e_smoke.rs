//! Real-window e2e smoke (ignored by default).
//!
//! Spawns the actual `halley` binary, lets it open a window on
//! `about:blank` (no network), and asserts the process stays alive for a
//! short grace period. Run with:
//!
//! ```text
//! cargo test -p halley-core --test e2e_smoke -- --ignored
//! ```
//!
//! Requires a display (Windows WebView2 / Linux WebKitGTK). Skipped in
//! default `cargo test --workspace` runs.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires a display; run manually with --ignored"]
fn binary_opens_window_and_stays_alive() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_halley"))
        .arg("about:blank")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn halley binary");

    // Give the event loop time to create the window and load about:blank.
    std::thread::sleep(Duration::from_millis(1500));

    match child.try_wait() {
        Ok(Some(status)) => panic!("halley exited early with status {status}"),
        Ok(None) => {} // still running — good
        Err(err) => panic!("failed to poll child: {err}"),
    }

    // Graceful kill; the loop should not have needed it.
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
#[ignore = "requires a display; run manually with --ignored"]
fn binary_rejects_unknown_scheme_before_opening() {
    let start = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_halley"))
        .arg("javascript:alert(1)")
        .output()
        .expect("failed to spawn halley binary");

    // Should exit non-zero quickly without opening a window.
    assert!(
        !output.status.success(),
        "expected non-zero exit for javascript: URL"
    );
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "rejection should be fast, took {:?}",
        start.elapsed()
    );
}
