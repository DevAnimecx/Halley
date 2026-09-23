//! `halley` binary entry point.
//!
//! Usage:
//!
//! ```text
//! halley [URL]
//! ```
//!
//! With no argument, opens the configured homepage (default
//! `about:blank` — no network). With an argument, the argument is
//! normalized exactly like address-bar input (URL or search).

use halley_common::{logging, Config};
use halley_core::{normalize_address, Application, SUBSYSTEM};

fn main() {
    logging::init();

    let mut args = std::env::args().skip(1);
    let config = Config::default();

    let input = match args.next() {
        Some(raw) => raw,
        None => config.browser.homepage.clone(),
    };

    if let Some(extra) = args.next() {
        halley_common::log_error!("unexpected argument: {extra}");
        std::process::exit(2);
    }

    let url = match normalize_address(&input, &config.browser.search_provider) {
        Ok(url) => url,
        Err(err) => {
            halley_common::log_error!("[{SUBSYSTEM}] cannot open {input:?}: {err}");
            std::process::exit(2);
        }
    };

    let mut app = Application::new(config.clone());
    if let Err(err) = app.ensure_profile() {
        halley_common::log_error!("[{SUBSYSTEM}] {err}");
        std::process::exit(1);
    }

    halley_common::log_info!("[{SUBSYSTEM}] opening {url}");
    if let Err(err) = app.run(&url) {
        halley_common::log_error!("[{SUBSYSTEM}] {err}");
        std::process::exit(1);
    }
}
