//! Conversation memory: messages, sessions, optional disk persistence.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::JerryError;
use crate::provenance::Provenance;

/// Schema version for conversation JSON files.
pub const CONVERSATION_SCHEMA_VERSION: u32 = 1;

/// Role of a chat message (wire-compatible subset of common APIs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    /// Fixed product instructions.
    System,
    /// Human user.
    User,
    /// Model assistant.
    Assistant,
}

/// One chat message with provenance (assistant → Model, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Chat role for the provider API.
    pub role: MessageRole,
    /// Message text (already privacy-prepared before send if needed).
    pub content: String,
    /// Who produced this message (defaults implied by role when loading).
    pub provenance: Provenance,
}

impl Message {
    /// System message from Halley.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            provenance: Provenance::System,
        }
    }

    /// User message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            provenance: Provenance::User,
        }
    }

    /// Assistant/model message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            provenance: Provenance::Model,
        }
    }
}

/// A single conversation thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    /// Opaque id (not a secret).
    pub id: String,
    /// Schema version of this struct.
    pub schema_version: u32,
    /// Ordered messages (oldest first).
    pub messages: Vec<Message>,
}

impl Conversation {
    /// New empty conversation with the given id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            schema_version: CONVERSATION_SCHEMA_VERSION,
            messages: Vec::new(),
        }
    }

    /// Append a message.
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Last `n` messages (for bounded context windows).
    pub fn tail(&self, n: usize) -> &[Message] {
        let start = self.messages.len().saturating_sub(n);
        &self.messages[start..]
    }
}

/// Persistable set of conversations (one active thread in Prompt #5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationStore {
    /// Schema version of the file envelope.
    pub schema_version: u32,
    /// All conversations.
    pub conversations: Vec<Conversation>,
    /// Id of the conversation currently shown in the UI, if any.
    pub active_id: Option<String>,
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self {
            schema_version: CONVERSATION_SCHEMA_VERSION,
            conversations: Vec::new(),
            active_id: None,
        }
    }
}

impl ConversationStore {
    /// Empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a new conversation and mark it active.
    pub fn start_new(&mut self, id: impl Into<String>) -> &Conversation {
        let id = id.into();
        let conv = Conversation::new(id.clone());
        self.conversations.push(conv);
        self.active_id = Some(id);
        self.active().expect("just pushed")
    }

    /// Active conversation, if any.
    pub fn active(&self) -> Option<&Conversation> {
        let id = self.active_id.as_ref()?;
        self.conversations.iter().find(|c| &c.id == id)
    }

    /// Mutable active conversation.
    pub fn active_mut(&mut self) -> Option<&mut Conversation> {
        let id = self.active_id.clone()?;
        self.conversations.iter_mut().find(|c| c.id == id)
    }

    /// Ensure an active conversation exists (creates `default_id` if not).
    pub fn ensure_active(&mut self, default_id: impl Into<String>) -> &Conversation {
        if self.active().is_none() {
            self.start_new(default_id);
        }
        self.active().expect("ensure_active")
    }

    /// Load from JSON bytes; rejects unknown schema versions.
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, JerryError> {
        let store: ConversationStore = serde_json::from_slice(bytes)
            .map_err(|err| JerryError::Storage(format!("parse conversations: {err}")))?;
        if store.schema_version != CONVERSATION_SCHEMA_VERSION {
            return Err(JerryError::Storage(format!(
                "unsupported conversation schema_version {}",
                store.schema_version
            )));
        }
        Ok(store)
    }

    /// Serialize to pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, JerryError> {
        serde_json::to_string_pretty(self)
            .map_err(|err| JerryError::Storage(format!("serialize conversations: {err}")))
    }

    /// Load from a file path (missing file → empty store).
    pub fn load_file(path: &Path) -> Result<Self, JerryError> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let bytes = std::fs::read(path)
            .map_err(|err| JerryError::Storage(format!("read conversations: {}", err.kind())))?;
        Self::from_json_slice(&bytes)
    }

    /// Write to a file path (creates parent dirs).
    pub fn save_file(&self, path: &Path) -> Result<(), JerryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| JerryError::Storage(format!("mkdir: {}", err.kind())))?;
        }
        let json = self.to_json_pretty()?;
        std::fs::write(path, json)
            .map_err(|err| JerryError::Storage(format!("write conversations: {}", err.kind())))
    }

    /// Path helper for tests / host wiring.
    pub fn default_path_under(dir: &Path) -> PathBuf {
        dir.join("jerry-conversations.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_active_creates_and_reuses() {
        let mut store = ConversationStore::new();
        let a = store.ensure_active("c1").id.clone();
        let b = store.ensure_active("c2").id.clone();
        assert_eq!(a, b);
        assert_eq!(store.conversations.len(), 1);
    }

    #[test]
    fn start_new_switches_active() {
        let mut store = ConversationStore::new();
        store.start_new("one");
        store.start_new("two");
        assert_eq!(store.active().unwrap().id, "two");
        assert_eq!(store.conversations.len(), 2);
    }

    #[test]
    fn schema_version_mismatch_is_rejected() {
        let mut store = ConversationStore::new();
        store.start_new("x");
        store.schema_version = 99;
        let json = store.to_json_pretty().unwrap();
        let err = ConversationStore::from_json_slice(json.as_bytes()).unwrap_err();
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn round_trip_json() {
        let mut store = ConversationStore::new();
        store.ensure_active("main");
        store.active_mut().unwrap().push(Message::user("hello"));
        let json = store.to_json_pretty().unwrap();
        let back = ConversationStore::from_json_slice(json.as_bytes()).unwrap();
        assert_eq!(back, store);
    }

    #[test]
    fn file_round_trip_temp() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("halley-jerry-conv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = ConversationStore::default_path_under(&dir);

        let mut store = ConversationStore::new();
        store.ensure_active("main");
        store.active_mut().unwrap().push(Message::system("sys"));
        store.save_file(&path).unwrap();

        let loaded = ConversationStore::load_file(&path).unwrap();
        assert_eq!(loaded, store);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_loads_empty() {
        let path = std::env::temp_dir().join(format!(
            "halley-jerry-missing-{}-nope.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let store = ConversationStore::load_file(&path).unwrap();
        assert!(store.conversations.is_empty());
    }

    #[test]
    fn tail_returns_last_n() {
        let mut c = Conversation::new("t");
        c.push(Message::user("1"));
        c.push(Message::user("2"));
        c.push(Message::user("3"));
        let tail = c.tail(2);
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].content, "2");
    }
}
