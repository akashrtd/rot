//! Session entry types for JSONL persistence.

use serde::{Deserialize, Serialize};

/// A single entry in a session JSONL file.
///
/// Each line in a session file is one `SessionEntry` serialized as JSON.
/// Uses `#[serde(tag = "type")]` for discriminated union format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionEntry {
    /// Session start marker (first line).
    #[serde(rename = "session/start")]
    SessionStart {
        id: String,
        timestamp: u64,
        cwd: String,
        model: String,
        provider: String,
    },

    /// A chat message (user, assistant, or system).
    #[serde(rename = "message")]
    Message {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        parent_id: Option<String>,
        timestamp: u64,
        role: String,
        content: serde_json::Value,
    },

    /// A tool invocation (part of an assistant response).
    #[serde(rename = "tool_call")]
    ToolCall {
        id: String,
        parent_id: String,
        timestamp: u64,
        name: String,
        arguments: serde_json::Value,
    },

    /// The result of a tool invocation.
    #[serde(rename = "tool_result")]
    ToolResult {
        id: String,
        call_id: String,
        timestamp: u64,
        output: String,
        is_error: bool,
    },

    /// A compaction marker indicating earlier messages were summarized.
    #[serde(rename = "compaction")]
    Compaction {
        id: String,
        timestamp: u64,
        summary: String,
        first_kept_id: String,
    },

    /// A branch point for alternative conversation paths.
    #[serde(rename = "branch")]
    Branch {
        id: String,
        from_id: String,
        timestamp: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
}

/// Metadata about a session (read from first line + computed stats).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    /// Session identifier.
    pub id: String,
    /// Unix timestamp when session was created.
    pub created_at: u64,
    /// Unix timestamp of last update.
    pub updated_at: u64,
    /// Optional title for the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Working directory.
    pub cwd: String,
    /// Model used.
    pub model: String,
    /// Provider used.
    pub provider: String,
    /// Number of message entries.
    pub message_count: usize,
}

/// Helper to get the ID from any entry type.
pub fn entry_id(entry: &SessionEntry) -> &str {
    match entry {
        SessionEntry::SessionStart { id, .. } => id,
        SessionEntry::Message { id, .. } => id,
        SessionEntry::ToolCall { id, .. } => id,
        SessionEntry::ToolResult { id, .. } => id,
        SessionEntry::Compaction { id, .. } => id,
        SessionEntry::Branch { id, .. } => id,
    }
}

/// Helper to get the timestamp from any entry type.
pub fn entry_timestamp(entry: &SessionEntry) -> u64 {
    match entry {
        SessionEntry::SessionStart { timestamp, .. } => *timestamp,
        SessionEntry::Message { timestamp, .. } => *timestamp,
        SessionEntry::ToolCall { timestamp, .. } => *timestamp,
        SessionEntry::ToolResult { timestamp, .. } => *timestamp,
        SessionEntry::Compaction { timestamp, .. } => *timestamp,
        SessionEntry::Branch { timestamp, .. } => *timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_start_serialization() {
        let entry = SessionEntry::SessionStart {
            id: "01HX123".to_string(),
            timestamp: 1234567890,
            cwd: "/home/user/project".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            provider: "anthropic".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"session/start\""));
        assert!(json.contains("\"id\":\"01HX123\""));

        let deserialized: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry_id(&deserialized), "01HX123");
    }

    #[test]
    fn test_message_entry_serialization() {
        let entry = SessionEntry::Message {
            id: "01HY456".to_string(),
            parent_id: Some("01HX123".to_string()),
            timestamp: 1234567891,
            role: "user".to_string(),
            content: serde_json::json!([{"type": "text", "text": "Hello"}]),
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"message\""));

        let deserialized: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry_id(&deserialized), "01HY456");
    }

    #[test]
    fn test_tool_call_entry() {
        let entry = SessionEntry::ToolCall {
            id: "tc_1".to_string(),
            parent_id: "01HY456".to_string(),
            timestamp: 1234567892,
            name: "read".to_string(),
            arguments: serde_json::json!({"path": "main.rs"}),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry_id(&deserialized), "tc_1");
    }

    #[test]
    fn test_tool_result_entry() {
        let entry = SessionEntry::ToolResult {
            id: "tr_1".to_string(),
            call_id: "tc_1".to_string(),
            timestamp: 1234567893,
            output: "fn main() {}".to_string(),
            is_error: false,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry_id(&deserialized), "tr_1");
    }

    #[test]
    fn test_compaction_entry() {
        let entry = SessionEntry::Compaction {
            id: "comp_1".to_string(),
            timestamp: 1234567894,
            summary: "User asked about main.rs".to_string(),
            first_kept_id: "01HY789".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"compaction\""));

        let deserialized: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry_id(&deserialized), "comp_1");
    }

    #[test]
    fn test_branch_entry() {
        let entry = SessionEntry::Branch {
            id: "br_1".to_string(),
            from_id: "01HY456".to_string(),
            timestamp: 1234567895,
            label: Some("alternative approach".to_string()),
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"branch\""));

        let deserialized: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry_id(&deserialized), "br_1");
    }

    #[test]
    fn test_entry_timestamp_helper() {
        let entry = SessionEntry::SessionStart {
            id: "test".to_string(),
            timestamp: 999,
            cwd: "/".to_string(),
            model: "test".to_string(),
            provider: "test".to_string(),
        };
        assert_eq!(entry_timestamp(&entry), 999);
    }

    #[test]
    fn test_jsonl_roundtrip() {
        // Simulate a JSONL file with multiple entries
        let entries = vec![
            SessionEntry::SessionStart {
                id: "s1".to_string(),
                timestamp: 1000,
                cwd: "/project".to_string(),
                model: "claude".to_string(),
                provider: "anthropic".to_string(),
            },
            SessionEntry::Message {
                id: "m1".to_string(),
                parent_id: None,
                timestamp: 1001,
                role: "user".to_string(),
                content: serde_json::json!([{"type": "text", "text": "Hello"}]),
            },
        ];

        // Serialize to JSONL
        let jsonl: String = entries
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect::<Vec<_>>()
            .join("\n");

        // Deserialize from JSONL
        let parsed: Vec<SessionEntry> = jsonl
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        assert_eq!(parsed.len(), 2);
        assert_eq!(entry_id(&parsed[0]), "s1");
        assert_eq!(entry_id(&parsed[1]), "m1");
    }
}
