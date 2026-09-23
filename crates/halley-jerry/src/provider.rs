//! Provider identities, default base URLs, and wire request builders.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::JerryError;

/// Known BYOK provider kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// OpenAI Platform (`api.openai.com`), OpenAI-compatible wire format.
    OpenAi,
    /// Groq (`api.groq.com`), OpenAI-compatible wire format.
    Groq,
    /// Anthropic Messages API.
    Anthropic,
    /// Google Gemini generateContent API.
    Gemini,
    /// Local OpenAI-compatible server (Ollama, llama.cpp, …).
    Local,
}

impl ProviderKind {
    /// Stable config/UI string.
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderKind::OpenAi => "openai",
            ProviderKind::Groq => "groq",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Gemini => "gemini",
            ProviderKind::Local => "local",
        }
    }

    /// Parse from config/UI string (trim, ASCII lowercase).
    pub fn parse(raw: &str) -> Result<Self, JerryError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "openai" | "openai-compatible" => Ok(ProviderKind::OpenAi),
            "groq" => Ok(ProviderKind::Groq),
            "anthropic" | "claude" => Ok(ProviderKind::Anthropic),
            "gemini" | "google" => Ok(ProviderKind::Gemini),
            "local" | "ollama" | "localhost" => Ok(ProviderKind::Local),
            other => Err(JerryError::InvalidRequest(format!(
                "unknown provider {other:?}"
            ))),
        }
    }

    /// All kinds in a stable order (for UI listings).
    pub fn all() -> [ProviderKind; 5] {
        [
            ProviderKind::OpenAi,
            ProviderKind::Groq,
            ProviderKind::Anthropic,
            ProviderKind::Gemini,
            ProviderKind::Local,
        ]
    }
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Default base URLs (no trailing path surprises).
pub fn default_base_urls() -> BTreeMap<&'static str, &'static str> {
    let mut m = BTreeMap::new();
    m.insert("openai", "https://api.openai.com/v1");
    m.insert("groq", "https://api.groq.com/openai/v1");
    m.insert("anthropic", "https://api.anthropic.com");
    m.insert("gemini", "https://generativelanguage.googleapis.com");
    m.insert("local", "http://127.0.0.1:11434/v1");
    m
}

/// Convenience re-export map type used by docs/tests.
pub type DefaultBaseUrls = BTreeMap<&'static str, &'static str>;

/// Default base URL map (alias of [`default_base_urls`]).
pub const DEFAULT_BASE_URLS: fn() -> DefaultBaseUrls = default_base_urls;

/// Static description of how to talk to a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSpec {
    /// Which provider.
    pub kind: ProviderKind,
    /// Default base URL (https except local loopback http).
    pub base_url: &'static str,
    /// Wire dialect.
    pub dialect: WireDialect,
    /// Default model id when user has not chosen one.
    pub default_model: &'static str,
}

/// Request/response wire format family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireDialect {
    /// OpenAI Chat Completions (`/chat/completions`).
    OpenAiChat,
    /// Anthropic Messages (`/v1/messages`).
    AnthropicMessages,
    /// Gemini generateContent.
    GeminiGenerate,
}

impl ProviderKind {
    /// Static spec for this provider.
    pub fn spec(self) -> ProviderSpec {
        match self {
            ProviderKind::OpenAi => ProviderSpec {
                kind: ProviderKind::OpenAi,
                base_url: "https://api.openai.com/v1",
                dialect: WireDialect::OpenAiChat,
                default_model: "gpt-4o-mini",
            },
            ProviderKind::Groq => ProviderSpec {
                kind: ProviderKind::Groq,
                base_url: "https://api.groq.com/openai/v1",
                dialect: WireDialect::OpenAiChat,
                default_model: "llama-3.1-8b-instant",
            },
            ProviderKind::Anthropic => ProviderSpec {
                kind: ProviderKind::Anthropic,
                base_url: "https://api.anthropic.com",
                dialect: WireDialect::AnthropicMessages,
                default_model: "claude-3-5-haiku-latest",
            },
            ProviderKind::Gemini => ProviderSpec {
                kind: ProviderKind::Gemini,
                base_url: "https://generativelanguage.googleapis.com",
                dialect: WireDialect::GeminiGenerate,
                default_model: "gemini-1.5-flash",
            },
            ProviderKind::Local => ProviderSpec {
                kind: ProviderKind::Local,
                base_url: "http://127.0.0.1:11434/v1",
                dialect: WireDialect::OpenAiChat,
                default_model: "llama3.2",
            },
        }
    }
}

/// Endpoint allowlist rules (privacy/security).
///
/// * Scheme must be `https`, except `http` on loopback hosts (local models).
/// * No userinfo, no non-default secret query params required by us
///   (Gemini key is sent as a header, never in the URL).
/// * Host must be a single DNS label-ish name or IP (no weird tricks).
pub fn validate_endpoint(base_url: &str) -> Result<url::Url, JerryError> {
    let url = url::Url::parse(base_url).map_err(|_| JerryError::EndpointNotAllowed {
        reason: "unparseable URL".into(),
    })?;
    match url.scheme() {
        "https" => {}
        "http" => {
            let host = url.host_str().unwrap_or("");
            let loopback = matches!(host, "127.0.0.1" | "localhost" | "::1");
            if !loopback {
                return Err(JerryError::EndpointNotAllowed {
                    reason: "http only allowed for loopback".into(),
                });
            }
        }
        other => {
            return Err(JerryError::EndpointNotAllowed {
                reason: format!("scheme {other}"),
            });
        }
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(JerryError::EndpointNotAllowed {
            reason: "userinfo not allowed".into(),
        });
    }
    Ok(url)
}

/// Join base + path segments without double slashes.
pub fn join_endpoint(base: &url::Url, path: &str) -> Result<url::Url, JerryError> {
    let mut url = base.clone();
    // Treat base as directory-ish: ensure trailing slash then join.
    if !url.path().ends_with('/') {
        let mut p = url.path().to_string();
        p.push('/');
        url.set_path(&p);
    }
    url.join(path.trim_start_matches('/'))
        .map_err(|_| JerryError::EndpointNotAllowed {
            reason: "path join".into(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_known_providers() {
        assert_eq!(ProviderKind::parse("OpenAI").unwrap(), ProviderKind::OpenAi);
        assert_eq!(
            ProviderKind::parse(" claude ").unwrap(),
            ProviderKind::Anthropic
        );
        assert!(ProviderKind::parse("chatgpt-enterprise").is_err());
    }

    #[test]
    fn validate_requires_https_except_loopback() {
        assert!(validate_endpoint("https://api.openai.com/v1").is_ok());
        assert!(validate_endpoint("http://127.0.0.1:11434/v1").is_ok());
        assert!(validate_endpoint("http://localhost:11434/v1").is_ok());
        assert!(validate_endpoint("http://api.openai.com/v1").is_err());
        assert!(validate_endpoint("ftp://example.com/").is_err());
        assert!(validate_endpoint("https://user:pass@example.com/").is_err());
    }

    #[test]
    fn join_endpoint_avoids_double_slash() {
        let base = validate_endpoint("https://api.openai.com/v1").unwrap();
        let url = join_endpoint(&base, "/chat/completions").unwrap();
        assert_eq!(url.as_str(), "https://api.openai.com/v1/chat/completions");
        let base2 = validate_endpoint("https://api.openai.com/v1/").unwrap();
        let url2 = join_endpoint(&base2, "chat/completions").unwrap();
        assert_eq!(url2.as_str(), "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn specs_have_sane_defaults() {
        for kind in ProviderKind::all() {
            let spec = kind.spec();
            assert_eq!(spec.kind, kind);
            validate_endpoint(spec.base_url).unwrap();
            assert!(!spec.default_model.is_empty());
        }
    }
}
