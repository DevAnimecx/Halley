//! Privacy-safe network logging helpers.
//!
//! Never log: `Authorization`, `Cookie`, `Set-Cookie`, bodies, or secret
//! query values. This module is the redaction layer callers must use.

use url::Url;

/// Replacement token for redacted material.
pub const REDACTED: &str = "REDACTED";

/// Headers whose values must never appear in logs.
pub fn is_sensitive_header(name_lower: &str) -> bool {
    matches!(
        name_lower,
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "set-cookie"
            | "x-api-key"
            | "x-auth-token"
    )
}

/// Redact a header value if the name is sensitive; otherwise return the
/// value unchanged (still never pass bodies here).
pub fn redact_header(name_lower: &str, value: &str) -> String {
    if is_sensitive_header(name_lower) || value_looks_like_secret(value) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

/// True when a value looks like a bearer token / API key even under an
/// innocuous header name.
fn value_looks_like_secret(value: &str) -> bool {
    let v = value.trim();
    if v.len() >= 20 && (v.starts_with("Bearer ") || v.starts_with("bearer ")) {
        return true;
    }
    false
}

/// Redact sensitive query parameters in a URL for logging.
///
/// Known secret-ish parameter names become `REDACTED`. The path and host
/// remain for debugging; query secrets do not.
pub fn redact_url_for_log(url: &Url) -> String {
    let mut out = url.clone();
    let secret_keys: &[&str] = &[
        "token",
        "access_token",
        "refresh_token",
        "id_token",
        "api_key",
        "apikey",
        "key",
        "secret",
        "password",
        "passwd",
        "session",
        "sid",
        "auth",
        "authorization",
        "code",
        "sig",
        "signature",
        "client_secret",
    ];
    let pairs: Vec<(String, String)> = out
        .query_pairs()
        .map(|(k, v)| {
            let lower = k.to_ascii_lowercase();
            if secret_keys.contains(&lower.as_str())
                || lower.contains("token")
                || lower.contains("secret")
                || lower.contains("password")
            {
                (k.into_owned(), REDACTED.to_string())
            } else {
                (k.into_owned(), v.into_owned())
            }
        })
        .collect();
    out.set_query(None);
    if !pairs.is_empty() {
        {
            let mut qp = out.query_pairs_mut();
            for (k, v) in pairs {
                qp.append_pair(&k, &v);
            }
        }
    }
    out.to_string()
}

/// Build a multi-line log representation of headers with redaction.
///
/// Values of sensitive headers are replaced; this is what tests assert on.
pub fn redact_headers_for_log(headers: &[(String, String)]) -> String {
    headers
        .iter()
        .map(|(name, value)| {
            let lower = name.to_ascii_lowercase();
            format!("{name}: {}", redact_header(&lower, value))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Log line for a request: method + redacted URL only (no headers/body).
pub fn request_log_line(method: &str, url: &Url) -> String {
    format!("{method} {}", redact_url_for_log(url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_and_cookie_are_redacted() {
        let headers = vec![
            (
                "Authorization".to_string(),
                "Bearer SUPER_SECRET".to_string(),
            ),
            ("Cookie".to_string(), "session=SUPER_SECRET".to_string()),
            ("Set-Cookie".to_string(), "session=SUPER_SECRET".to_string()),
            ("Accept".to_string(), "text/html".to_string()),
        ];
        let rendered = redact_headers_for_log(&headers);
        assert!(!rendered.contains("SUPER_SECRET"));
        assert!(rendered.contains("Authorization: REDACTED"));
        assert!(rendered.contains("Cookie: REDACTED"));
        assert!(rendered.contains("Set-Cookie: REDACTED"));
        assert!(rendered.contains("Accept: text/html"));
    }

    #[test]
    fn secret_query_params_are_redacted_in_urls() {
        let url =
            Url::parse("https://example.com/login?token=SUPER_SECRET&q=shoes&page=2").unwrap();
        let logged = redact_url_for_log(&url);
        assert!(!logged.contains("SUPER_SECRET"));
        assert!(logged.contains("token=REDACTED"));
        assert!(logged.contains("q=shoes"));
        assert!(logged.contains("https://example.com/login"));
    }

    #[test]
    fn bearer_under_custom_header_is_redacted() {
        assert_eq!(
            redact_header("x-weird", "Bearer SUPER_SECRET_TOKEN_VALUE"),
            REDACTED
        );
    }

    #[test]
    fn request_log_line_has_no_query_secret() {
        let url = Url::parse("https://api.example/v1?api_key=SUPER_SECRET").unwrap();
        let line = request_log_line("GET", &url);
        assert!(!line.contains("SUPER_SECRET"));
        assert!(line.starts_with("GET "));
    }
}
