//! Chrome IPC parsing and chrome-state script generation.
//!
//! The embedded chrome page (trusted, compiled into the binary) talks to
//! Rust over wry's `window.ipc.postMessage`. Content pages have **no** IPC
//! handler; only the chrome web view may emit [`ChromeAction`]s.

use crate::types::{
    ChromeAction, ChromeState, ChromeTabState, JerryChromeMessage, JerryChromeState,
};

/// Parse a raw IPC body from the chrome web view into a [`ChromeAction`].
///
/// Protocol (space-separated tokens, UTF-8):
///
/// ```text
/// back | forward | reload | stop | newtab | nexttab | prevtab
/// reopentab | focusaddress
/// navigate <raw-input>
/// closetab <tab-id>
/// activatetab <tab-id>
/// duplicatetab <tab-id>
/// closeothertabs <tab-id>
/// closetabsright <tab-id>
/// movetab <tab-id> <index>
/// jerrytoggle | jerrystop | jerrynew | jerryclear
/// jerryenable | jerrydisable | jerrytest | jerryclearkey
/// jerrysend <text>
/// jerryprovider <id>
/// jerrymodel <id>
/// jerrykey <secret>   (never logged by engine or core)
/// ```
///
/// Tab ids are decimal strings of `TabId` (parsed by core, not here).
/// Returns `None` for unknown or malformed bodies.
pub fn parse_chrome_message(body: &str) -> Option<ChromeAction> {
    let body = body.trim();
    match body {
        "back" => Some(ChromeAction::Back),
        "forward" => Some(ChromeAction::Forward),
        "reload" => Some(ChromeAction::Reload),
        "stop" => Some(ChromeAction::Stop),
        "newtab" => Some(ChromeAction::NewTab),
        "nexttab" => Some(ChromeAction::NextTab),
        "prevtab" => Some(ChromeAction::PrevTab),
        "reopentab" => Some(ChromeAction::ReopenClosedTab),
        "focusaddress" => Some(ChromeAction::FocusAddressBar),
        "jerrytoggle" => Some(ChromeAction::JerryToggle),
        "jerrystop" => Some(ChromeAction::JerryStop),
        "jerrynew" => Some(ChromeAction::JerryNewConversation),
        "jerryclear" => Some(ChromeAction::JerryClear),
        "jerryenable" => Some(ChromeAction::JerryEnable),
        "jerrydisable" => Some(ChromeAction::JerryDisable),
        "jerrytest" => Some(ChromeAction::JerryTestProvider),
        "jerryclearkey" => Some(ChromeAction::JerryClearKey),
        _ => parse_prefixed(body),
    }
}

fn parse_prefixed(body: &str) -> Option<ChromeAction> {
    let (verb, rest) = body.split_once(' ')?;
    let rest = rest.trim();
    match verb {
        "navigate" => {
            if rest.is_empty() {
                None
            } else {
                Some(ChromeAction::Navigate(rest.to_string()))
            }
        }
        "closetab" => id_action(rest, ChromeAction::CloseTab),
        "activatetab" => id_action(rest, ChromeAction::ActivateTab),
        "duplicatetab" => id_action(rest, ChromeAction::DuplicateTab),
        "closeothertabs" => id_action(rest, ChromeAction::CloseOtherTabs),
        "closetabsright" => id_action(rest, ChromeAction::CloseTabsToRight),
        "movetab" => {
            let (id, idx) = rest.split_once(' ')?;
            let id = id.trim();
            let idx = idx.trim().parse::<usize>().ok()?;
            if id.is_empty() {
                None
            } else {
                Some(ChromeAction::MoveTab {
                    tab_id: id.to_string(),
                    to_index: idx,
                })
            }
        }
        "jerrysend" => {
            if rest.is_empty() {
                None
            } else {
                Some(ChromeAction::JerrySend(rest.to_string()))
            }
        }
        "jerryprovider" => {
            if rest.is_empty() {
                None
            } else {
                Some(ChromeAction::JerrySetProvider(rest.to_string()))
            }
        }
        "jerrymodel" => {
            if rest.is_empty() {
                None
            } else {
                Some(ChromeAction::JerrySetModel(rest.to_string()))
            }
        }
        "jerrykey" => {
            if rest.is_empty() {
                None
            } else {
                Some(ChromeAction::JerrySetKey(rest.to_string()))
            }
        }
        _ => None,
    }
}

fn id_action(rest: &str, f: impl FnOnce(String) -> ChromeAction) -> Option<ChromeAction> {
    if rest.is_empty() {
        None
    } else {
        Some(f(rest.to_string()))
    }
}

/// Build a JavaScript snippet that pushes [`ChromeState`] into the chrome
/// page via `window.__halleyUpdateChrome(state)`.
pub fn chrome_state_script(state: &ChromeState) -> String {
    let mut tabs_json = String::from("[");
    for (i, tab) in state.tabs.iter().enumerate() {
        if i > 0 {
            tabs_json.push(',');
        }
        tabs_json.push_str(&chrome_tab_json(tab));
    }
    tabs_json.push(']');

    format!(
        "window.__halleyUpdateChrome({{\"url\":{},\"title\":{},\"canGoBack\":{},\"canGoForward\":{},\"loading\":{},\"tabs\":{},\"activeTabId\":{},\"jerry\":{}}});",
        json_string(&state.url),
        json_string(&state.title),
        state.can_go_back,
        state.can_go_forward,
        state.loading,
        tabs_json,
        json_string(&state.active_tab_id),
        jerry_json(&state.jerry),
    )
}

fn jerry_json(jerry: &JerryChromeState) -> String {
    let mut messages = String::from("[");
    for (i, msg) in jerry.messages.iter().enumerate() {
        if i > 0 {
            messages.push(',');
        }
        messages.push_str(&jerry_message_json(msg));
    }
    messages.push(']');
    let error = match &jerry.error {
        Some(e) => json_string(e),
        None => "null".to_string(),
    };
    format!(
        "{{\"open\":{},\"enabled\":{},\"provider\":{},\"model\":{},\"hasApiKey\":{},\"status\":{},\"messages\":{},\"contextNote\":{},\"error\":{}}}",
        jerry.open,
        jerry.enabled,
        json_string(&jerry.provider),
        json_string(&jerry.model),
        jerry.has_api_key,
        json_string(&jerry.status),
        messages,
        json_string(&jerry.context_note),
        error,
    )
}

fn jerry_message_json(msg: &JerryChromeMessage) -> String {
    format!(
        "{{\"role\":{},\"content\":{},\"streaming\":{}}}",
        json_string(&msg.role),
        json_string(&msg.content),
        msg.streaming,
    )
}

fn chrome_tab_json(tab: &ChromeTabState) -> String {
    format!(
        "{{\"id\":{},\"title\":{},\"active\":{},\"loading\":{}}}",
        json_string(&tab.id),
        json_string(&tab.title),
        tab.active,
        tab.loading,
    )
}

/// Serialize `value` as a JavaScript/JSON string literal.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            // Line separators are legal JSON but terminate JS strings.
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_commands() {
        assert_eq!(parse_chrome_message("back"), Some(ChromeAction::Back));
        assert_eq!(
            parse_chrome_message("forward\n"),
            Some(ChromeAction::Forward)
        );
        assert_eq!(
            parse_chrome_message("  reload  "),
            Some(ChromeAction::Reload)
        );
        assert_eq!(parse_chrome_message("newtab"), Some(ChromeAction::NewTab));
        assert_eq!(parse_chrome_message("stop"), Some(ChromeAction::Stop));
        assert_eq!(
            parse_chrome_message("reopentab"),
            Some(ChromeAction::ReopenClosedTab)
        );
        assert_eq!(
            parse_chrome_message("focusaddress"),
            Some(ChromeAction::FocusAddressBar)
        );
    }

    #[test]
    fn parses_navigate_with_url() {
        assert_eq!(
            parse_chrome_message("navigate https://example.com/a?b=1"),
            Some(ChromeAction::Navigate(
                "https://example.com/a?b=1".to_string()
            ))
        );
    }

    #[test]
    fn parses_tab_id_commands() {
        assert_eq!(
            parse_chrome_message("closetab 3"),
            Some(ChromeAction::CloseTab("3".into()))
        );
        assert_eq!(
            parse_chrome_message("activatetab 12"),
            Some(ChromeAction::ActivateTab("12".into()))
        );
        assert_eq!(
            parse_chrome_message("duplicatetab 4"),
            Some(ChromeAction::DuplicateTab("4".into()))
        );
        assert_eq!(
            parse_chrome_message("closeothertabs 1"),
            Some(ChromeAction::CloseOtherTabs("1".into()))
        );
        assert_eq!(
            parse_chrome_message("closetabsright 2"),
            Some(ChromeAction::CloseTabsToRight("2".into()))
        );
        assert_eq!(
            parse_chrome_message("movetab 5 0"),
            Some(ChromeAction::MoveTab {
                tab_id: "5".into(),
                to_index: 0
            })
        );
    }

    #[test]
    fn rejects_unknown_and_empty_payloads() {
        assert_eq!(parse_chrome_message(""), None);
        assert_eq!(parse_chrome_message("javascript:alert(1)"), None);
        assert_eq!(parse_chrome_message("navigate"), None);
        assert_eq!(parse_chrome_message("navigate    "), None);
        assert_eq!(parse_chrome_message("closetab"), None);
        assert_eq!(parse_chrome_message("closetab "), None);
        assert_eq!(parse_chrome_message("movetab 1 notanumber"), None);
        assert_eq!(parse_chrome_message("evil verb args"), None);
    }

    #[test]
    fn chrome_state_script_embeds_tabs_and_escaped_fields() {
        let state = ChromeState {
            url: "https://example.com/".into(),
            title: "Say \"hi\"\\there".into(),
            can_go_back: true,
            can_go_forward: false,
            loading: true,
            tabs: vec![
                ChromeTabState {
                    id: "1".into(),
                    title: "One".into(),
                    active: false,
                    loading: false,
                },
                ChromeTabState {
                    id: "2".into(),
                    title: "Two".into(),
                    active: true,
                    loading: true,
                },
            ],
            active_tab_id: "2".into(),
            ..ChromeState::default()
        };
        let script = chrome_state_script(&state);
        assert!(script.starts_with("window.__halleyUpdateChrome({"));
        assert!(script.contains("\"url\":\"https://example.com/\""));
        assert!(script.contains("\"canGoBack\":true"));
        assert!(script.contains("\"loading\":true"));
        assert!(script.contains("\"activeTabId\":\"2\""));
        assert!(script.contains("\"id\":\"1\""));
        assert!(script.contains("\"id\":\"2\""));
        assert!(script.contains("\"active\":true"));
        assert!(script.contains("\"jerry\":{"));
        assert!(script.ends_with("});"));
    }

    #[test]
    fn chrome_state_script_embeds_jerry_panel_fields() {
        let state = ChromeState {
            jerry: JerryChromeState {
                open: true,
                enabled: true,
                provider: "openai".into(),
                model: "gpt-4o-mini".into(),
                has_api_key: true,
                status: "idle".into(),
                messages: vec![JerryChromeMessage {
                    role: "user".into(),
                    content: "hi \"there\"".into(),
                    streaming: false,
                }],
                context_note: "2 sources".into(),
                error: Some("bad \"key\"".into()),
            },
            ..ChromeState::default()
        };
        let script = chrome_state_script(&state);
        assert!(
            script.contains("\"jerry\":{\"open\":true,\"enabled\":true,\"provider\":\"openai\"")
        );
        assert!(script.contains("\"hasApiKey\":true"));
        assert!(script.contains("\"content\":\"hi \\\"there\\\"\""));
        assert!(script.contains("\"error\":\"bad \\\"key\\\"\""));
    }

    #[test]
    fn parses_jerry_verbs_and_prefixed_actions() {
        assert_eq!(
            parse_chrome_message("jerrytoggle"),
            Some(ChromeAction::JerryToggle)
        );
        assert_eq!(
            parse_chrome_message("jerrysend what is this page"),
            Some(ChromeAction::JerrySend("what is this page".into()))
        );
        assert_eq!(
            parse_chrome_message("jerryprovider anthropic"),
            Some(ChromeAction::JerrySetProvider("anthropic".into()))
        );
        assert_eq!(parse_chrome_message("jerrysend"), None);
        assert_eq!(
            parse_chrome_message("jerrykey super-secret"),
            Some(ChromeAction::JerrySetKey("super-secret".into()))
        );
    }

    #[test]
    fn json_string_escapes_line_separators_and_controls() {
        assert_eq!(json_string("a\u{2028}b\u{2029}c"), "\"a\\u2028b\\u2029c\"");
        assert_eq!(json_string("\u{0001}"), "\"\\u0001\"");
        assert_eq!(json_string("\u{007f}"), "\"\\u007f\"");
    }

    #[test]
    fn json_string_preserves_normal_unicode() {
        assert_eq!(json_string("héllo — 日本語"), "\"héllo — 日本語\"");
    }
}
