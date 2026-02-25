//! Core message types used throughout the rot system.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// Unique message identifier based on ULID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);

impl MessageId {
    /// Generate a new unique message ID.
    pub fn new() -> Self {
        Self(ulid::Ulid::new().to_string())
    }

    /// Create a MessageId from an existing string.
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the inner string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The role of a message sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User input
    User,
    /// AI assistant response
    Assistant,
    /// Tool execution result
    Tool,
    /// System prompt
    System,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::Tool => write!(f, "tool"),
            Role::System => write!(f, "system"),
        }
    }
}

/// A block of content within a message.
///
/// Messages can contain multiple content blocks of different types,
/// supporting rich interactions with tool calls and results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    /// Plain text content.
    #[serde(rename = "text")]
    Text { text: String },

    /// Base64-encoded image.
    #[serde(rename = "image")]
    Image {
        data: String,
        mime_type: String,
    },

    /// A tool invocation by the assistant.
    #[serde(rename = "tool_call")]
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },

    /// The result of a tool invocation.
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_call_id: String,
        content: String,
        is_error: bool,
    },

    /// Model thinking/reasoning (extended thinking).
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
}

/// A single message in a conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier.
    pub id: MessageId,
    /// Who sent this message.
    pub role: Role,
    /// Content blocks within the message.
    pub content: Vec<ContentBlock>,
    /// Unix timestamp (seconds since epoch).
    pub timestamp: u64,
    /// Parent message ID (for branching conversations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<MessageId>,
}

impl Message {
    /// Get the current Unix timestamp in seconds.
    fn now_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Create a new user message from text.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: text.into(),
            }],
            timestamp: Self::now_timestamp(),
            parent_id: None,
        }
    }

    /// Create a new assistant message from content blocks.
    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::Assistant,
            content,
            timestamp: Self::now_timestamp(),
            parent_id: None,
        }
    }

    /// Create a new system message from text.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::System,
            content: vec![ContentBlock::Text {
                text: text.into(),
            }],
            timestamp: Self::now_timestamp(),
            parent_id: None,
        }
    }

    /// Create a tool result message.
    pub fn tool_result(
        tool_call_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_call_id: tool_call_id.into(),
                content: content.into(),
                is_error,
            }],
            timestamp: Self::now_timestamp(),
            parent_id: None,
        }
    }

    /// Set the parent message ID.
    pub fn with_parent(mut self, parent_id: MessageId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Get the text content of this message (concatenated text blocks).
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_id_unique() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_message_id_from_string() {
        let id = MessageId::from_string("test-id");
        assert_eq!(id.as_str(), "test-id");
    }

    #[test]
    fn test_user_message() {
        let msg = Message::user("Hello, world!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.text(), "Hello, world!");
        assert!(msg.parent_id.is_none());
        assert!(msg.timestamp > 0);
    }

    #[test]
    fn test_assistant_message() {
        let msg = Message::assistant(vec![ContentBlock::Text {
            text: "Hi there!".to_string(),
        }]);
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.text(), "Hi there!");
    }

    #[test]
    fn test_system_message() {
        let msg = Message::system("You are a helpful assistant.");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.text(), "You are a helpful assistant.");
    }

    #[test]
    fn test_tool_result_message() {
        let msg = Message::tool_result("call-1", "file contents here", false);
        assert_eq!(msg.role, Role::Tool);
        match &msg.content[0] {
            ContentBlock::ToolResult {
                tool_call_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_call_id, "call-1");
                assert_eq!(content, "file contents here");
                assert!(!is_error);
            }
            _ => panic!("Expected ToolResult content block"),
        }
    }

    #[test]
    fn test_message_with_parent() {
        let parent = Message::user("first");
        let parent_id = parent.id.clone();
        let child = Message::user("second").with_parent(parent_id.clone());
        assert_eq!(child.parent_id, Some(parent_id));
    }

    #[test]
    fn test_role_serialization() {
        let json = serde_json::to_string(&Role::User).unwrap();
        assert_eq!(json, "\"user\"");

        let role: Role = serde_json::from_str("\"assistant\"").unwrap();
        assert_eq!(role, Role::Assistant);
    }

    #[test]
    fn test_content_block_serialization() {
        let block = ContentBlock::Text {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"hello\""));

        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, block);
    }

    #[test]
    fn test_tool_call_block_serialization() {
        let block = ContentBlock::ToolCall {
            id: "tc_1".to_string(),
            name: "read".to_string(),
            arguments: serde_json::json!({"path": "main.rs"}),
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, block);
    }

    #[test]
    fn test_message_serialization_roundtrip() {
        let msg = Message::user("Test message");
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, msg.role);
        assert_eq!(deserialized.text(), msg.text());
        assert_eq!(deserialized.id, msg.id);
    }

    #[test]
    fn test_multi_block_text() {
        let msg = Message {
            id: MessageId::new(),
            role: Role::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "Hello ".to_string(),
                },
                ContentBlock::ToolCall {
                    id: "tc_1".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::Value::Null,
                },
                ContentBlock::Text {
                    text: "world".to_string(),
                },
            ],
            timestamp: 0,
            parent_id: None,
        };
        assert_eq!(msg.text(), "Hello world");
    }
}
