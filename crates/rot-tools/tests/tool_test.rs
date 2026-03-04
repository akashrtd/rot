use rot_tools::{ToolRegistry, ToolContext, ToolResult, register_all};

#[test]
fn test_registry_new() {
    let registry = ToolRegistry::new();
    assert!(registry.names().is_empty());
}

#[test]
fn test_registry_default() {
    let registry = ToolRegistry::default();
    assert!(registry.names().is_empty());
}

#[test]
fn test_register_all_populates_registry() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    
    let names = registry.names();
    assert!(!names.is_empty());
    assert!(names.contains(&"read".to_string()));
    assert!(names.contains(&"write".to_string()));
    assert!(names.contains(&"bash".to_string()));
    assert!(names.contains(&"grep".to_string()));
    assert!(names.contains(&"glob".to_string()));
    assert!(names.contains(&"edit".to_string()));
}

#[test]
fn test_registry_get_existing() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    
    let tool = registry.get("read");
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().name(), "read");
}

#[test]
fn test_registry_get_nonexistent() {
    let registry = ToolRegistry::new();
    let tool = registry.get("nonexistent");
    assert!(tool.is_none());
}

#[test]
fn test_registry_names_contains_all() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    
    let names = registry.names();
    assert!(names.contains(&"read".to_string()));
    assert!(names.contains(&"write".to_string()));
    assert!(names.contains(&"bash".to_string()));
}

#[test]
fn test_tool_context_default() {
    let ctx = ToolContext::default();
    assert!(!ctx.working_dir.to_string_lossy().is_empty());
    assert!(ctx.session_id.is_empty());
    assert_eq!(ctx.timeout, std::time::Duration::from_secs(120));
}

#[test]
fn test_tool_result_success() {
    let result = ToolResult::success("output");
    assert!(!result.is_error);
    assert_eq!(result.output, "output");
}

#[test]
fn test_tool_result_error() {
    let result = ToolResult::error("error message");
    assert!(result.is_error);
    assert_eq!(result.output, "error message");
}

#[test]
fn test_tool_result_success_with_metadata() {
    let metadata = serde_json::json!({"bytes": 100});
    let result = ToolResult::success_with_metadata("output", metadata.clone());
    assert!(!result.is_error);
    assert_eq!(result.output, "output");
    assert_eq!(result.metadata, metadata);
}

#[test]
fn test_tool_result_is_success() {
    let success = ToolResult::success("ok");
    assert!(!success.is_error);
    
    let error = ToolResult::error("fail");
    assert!(error.is_error);
}

#[test]
fn test_read_tool_name() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    let tool = registry.get("read").unwrap();
    assert_eq!(tool.name(), "read");
}

#[test]
fn test_read_tool_description() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    let tool = registry.get("read").unwrap();
    assert!(!tool.description().is_empty());
}

#[test]
fn test_read_tool_parameters_schema() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    let tool = registry.get("read").unwrap();
    let schema = tool.parameters_schema();
    assert!(schema["properties"]["path"].is_object());
}

#[test]
fn test_write_tool_name() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    let tool = registry.get("write").unwrap();
    assert_eq!(tool.name(), "write");
}

#[test]
fn test_bash_tool_name() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    let tool = registry.get("bash").unwrap();
    assert_eq!(tool.name(), "bash");
}

#[test]
fn test_grep_tool_name() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    let tool = registry.get("grep").unwrap();
    assert_eq!(tool.name(), "grep");
}

#[test]
fn test_glob_tool_name() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    let tool = registry.get("glob").unwrap();
    assert_eq!(tool.name(), "glob");
}

#[test]
fn test_edit_tool_name() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    let tool = registry.get("edit").unwrap();
    assert_eq!(tool.name(), "edit");
}

#[test]
fn test_task_tool_name() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    let tool = registry.get("task").unwrap();
    assert_eq!(tool.name(), "task");
}

#[test]
fn test_all_tools_have_descriptions() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    
    for name in registry.names() {
        let tool = registry.get(&name).unwrap();
        assert!(!tool.description().is_empty(), "Tool '{}' has empty description", name);
    }
}

#[test]
fn test_all_tools_have_schemas() {
    let mut registry = ToolRegistry::new();
    register_all(&mut registry);
    
    for name in registry.names() {
        let tool = registry.get(&name).unwrap();
        let schema = tool.parameters_schema();
        assert!(schema.is_object(), "Tool '{}' has invalid schema", name);
    }
}
