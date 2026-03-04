use rot_core::{ContentBlock, Message, MessageId, Role};

#[test]
fn test_message_id_uniqueness() {
    let id1 = MessageId::new();
    let id2 = MessageId::new();
    assert_ne!(id1, id2);
}

#[test]
fn test_message_id_from_string() {
    let id = MessageId::from_string("custom-id-123");
    assert_eq!(id.as_str(), "custom-id-123");
}

#[test]
fn test_message_id_display() {
    let id = MessageId::from_string("test-id");
    let display = format!("{}", id);
    assert_eq!(display, "test-id");
}

#[test]
fn test_role_equality() {
    assert_eq!(Role::User, Role::User);
    assert_eq!(Role::Assistant, Role::Assistant);
    assert_ne!(Role::User, Role::Assistant);
    assert_ne!(Role::Tool, Role::System);
}

#[test]
fn test_role_display() {
    assert_eq!(format!("{}", Role::User), "user");
    assert_eq!(format!("{}", Role::Assistant), "assistant");
    assert_eq!(format!("{}", Role::Tool), "tool");
    assert_eq!(format!("{}", Role::System), "system");
}

#[test]
fn test_role_serialization() {
    let json = serde_json::to_string(&Role::User).unwrap();
    assert_eq!(json, "\"user\"");
    
    let json = serde_json::to_string(&Role::Assistant).unwrap();
    assert_eq!(json, "\"assistant\"");
}

#[test]
fn test_role_deserialization() {
    let role: Role = serde_json::from_str("\"user\"").unwrap();
    assert_eq!(role, Role::User);
    
    let role: Role = serde_json::from_str("\"assistant\"").unwrap();
    assert_eq!(role, Role::Assistant);
}

#[test]
fn test_user_message_creation() {
    let msg = Message::user("Hello, world!");
    assert_eq!(msg.role, Role::User);
    assert_eq!(msg.text(), "Hello, world!");
    assert!(msg.parent_id.is_none());
    assert!(msg.timestamp > 0);
}

#[test]
fn test_assistant_message_creation() {
    let msg = Message::assistant(vec![ContentBlock::Text {
        text: "Hi there!".to_string(),
    }]);
    assert_eq!(msg.role, Role::Assistant);
    assert_eq!(msg.text(), "Hi there!");
}

#[test]
fn test_system_message_creation() {
    let msg = Message::system("You are helpful.");
    assert_eq!(msg.role, Role::System);
    assert_eq!(msg.text(), "You are helpful.");
}

#[test]
fn test_tool_result_message() {
    let msg = Message::tool_result("call-123", "result", false);
    assert_eq!(msg.role, Role::Tool);
    assert!(matches!(&msg.content[0], ContentBlock::ToolResult { .. }));
}

#[test]
fn test_tool_result_with_metadata() {
    let metadata = serde_json::json!({"bytes": 100});
    let msg = Message::tool_result_with_metadata("call-456", "content", true, metadata.clone());
    
    match &msg.content[0] {
        ContentBlock::ToolResult { tool_call_id, content, is_error, metadata: meta } => {
            assert_eq!(tool_call_id, "call-456");
            assert_eq!(content, "content");
            assert!(*is_error);
            assert_eq!(meta, &metadata);
        }
        _ => panic!("Expected ToolResult"),
    }
}

#[test]
fn test_message_with_parent() {
    let parent = Message::user("parent");
    let parent_id = parent.id.clone();
    let child = Message::user("child").with_parent(parent_id.clone());
    assert_eq!(child.parent_id, Some(parent_id));
}

#[test]
fn test_message_text_multiple_blocks() {
    let msg = Message::assistant(vec![
        ContentBlock::Text { text: "Hello ".to_string() },
        ContentBlock::Text { text: "World".to_string() },
        ContentBlock::ToolCall {
            id: "tc1".to_string(),
            name: "read".to_string(),
            arguments: serde_json::json!({}),
        },
        ContentBlock::Text { text: "!".to_string() },
    ]);
    assert_eq!(msg.text(), "Hello World!");
}

#[test]
fn test_content_block_text_serialization() {
    let block = ContentBlock::Text { text: "test".to_string() };
    let json = serde_json::to_string(&block).unwrap();
    assert!(json.contains("\"type\":\"text\""));
    assert!(json.contains("\"text\":\"test\""));
}

#[test]
fn test_content_block_tool_call_serialization() {
    let block = ContentBlock::ToolCall {
        id: "tc1".to_string(),
        name: "read".to_string(),
        arguments: serde_json::json!({"path": "/tmp"}),
    };
    let json = serde_json::to_string(&block).unwrap();
    assert!(json.contains("\"type\":\"tool_call\""));
    assert!(json.contains("\"id\":\"tc1\""));
}

#[test]
fn test_message_clone() {
    let msg = Message::user("original");
    let cloned = msg.clone();
    assert_eq!(msg.text(), cloned.text());
    assert_eq!(msg.role, cloned.role);
}

#[test]
fn test_message_serialization() {
    let msg = Message::user("test");
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"role\":\"user\""));
    assert!(json.contains("\"content\""));
}
