//! Jerry — conversational intelligence foundation for Halley.
//!
//! Prompt #5 scope (**IMPLEMENTED** unless noted):
//!
//! * Conversation memory (messages with provenance, optional persistence)
//! * BYOK provider adapters: OpenAI-compatible, Groq, Anthropic, Gemini,
//!   and local OpenAI-compatible endpoints
//! * Streaming and non-streaming completions with cooperative cancellation
//! * Context assembly with token budgeting ([`halley_jerry_opt`])
//! * Privacy pipeline before any egress (provenance framing, redaction)
//! * Structured responses (text + stop reason + context stats)
//! * Read-only tool **contracts** via [`halley_jerry_mcp`] (no executor)
//!
//! **NOT IMPLEMENTED** in this crate: autonomous planning, tool execution
//! loops, multi-model routing, page DOM extraction, OS keychain storage.
//!
//! Safety reminders (ADR-005): webpage content is untrusted data, never
//! instructions; API keys never appear in logs or [`std::fmt::Debug`].

pub mod context;
pub mod conversation;
pub mod credentials;
pub mod error;
pub mod privacy;
pub mod provenance;
pub mod provider;
pub mod response;
pub mod runtime;
pub mod transport;

pub use context::{BrowserPageContext, ContextBundle, PageContextProvider};
pub use conversation::{
    Conversation, ConversationStore, Message, MessageRole, CONVERSATION_SCHEMA_VERSION,
};
pub use credentials::{CredentialStore, ProviderCredential};
pub use error::JerryError;
pub use privacy::{prepare_system_prompt, redact_secrets_in_text, PRIVACY_SYSTEM_SUFFIX};
pub use provenance::Provenance;
pub use provider::{ProviderKind, ProviderSpec, DEFAULT_BASE_URLS};
pub use response::{StopReason, StructuredResponse};
pub use runtime::{JerryRequest, JerryRuntime, JerrySettings};
pub use transport::{
    MockTransport, ProviderRequest, ProviderTransport, RecordedCall, UreqTransport,
};

/// Identifier for this subsystem, used by diagnostics and logging.
pub const SUBSYSTEM: &str = "halley-jerry";
