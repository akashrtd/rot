# Research & Implementation Plan: Fix Issues & Comprehensive Testing

**Date:** 2026-03-04  
**Status:** Research & Planning  
**Objective:** Fix all identified issues and implement comprehensive test coverage

---

## Executive Summary

This document provides a detailed research analysis of the issues identified during E2E testing, a comprehensive implementation plan to fix them, and a complete test strategy with specific test cases for all features.

### Scope
- **Root Cause Analysis:** Deep dive into 8 identified issues
- **Implementation Plan:** Step-by-step fixes with code changes
- **Test Strategy:** Unit, integration, and E2E test specifications
- **Timeline:** Estimated effort and priority

---

## Part 1: Root Cause Analysis

### Issue #1: Task Tool Timeout in Non-Interactive Mode

**Severity:** 🔴 Critical  
**Location:** `crates/rot-tools/src/builtin/task.rs`  
**Test File:** `crates/rot-tools/src/builtin/task.rs:207-225`

**Symptoms:**
- Task tool hangs indefinitely in exec mode
- No timeout error message
- Process never completes

**Root Cause:**
```rust
// From task.rs:111
let result = runner.run_task(TaskRequest { agent, prompt: params.prompt }).await?;
```

The `run_task` method waits for approval callback when tools need permission, but in non-interactive mode:
1. No approval callback is registered
2. Approval request waits forever for user response
3. No timeout is set on the approval future

**Evidence:**
- `ApprovalCallback` type in `rot-core/src/agent.rs:87-94`
- No default approval callback in exec mode
- `ApprovalPolicy::PerTool` requires interactive input

**Fix Required:**
1. Add timeout wrapper around approval callback
2. Provide default approval callback for exec mode
3. Add `--auto-approve` flag for non-interactive runs

---

### Issue #2: RLM Mode Timeout

**Severity:** 🔴 Critical  
**Location:** `crates/rot-rlm/src/engine.rs`  
**Test File:** `crates/rot-cli/tests/rlm_exec_test.rs`

**Symptoms:**
- RLM mode times out even with simple context
- No progress indication in exec mode
- No intermediate output until completion

**Root Cause:**
1. **No Progress Callbacks:** RLM engine doesn't emit progress events in exec mode
2. **Timeout Too Short:** Default timeout may be insufficient for complex iterations
3. **No Output Buffering:** Results only shown after completion

**Evidence from code:**
```rust
// exec.rs:154
let mut engine = rot_rlm::RlmEngine::new(config, agent.clone());
let report = engine.process_with_report(prompt, ctx_path).await?;
```

No event callbacks registered for progress reporting.

**Fix Required:**
1. Add progress callback to RLM engine in exec mode
2. Increase default timeout or make configurable
3. Stream intermediate results to stdout in JSON mode

---

### Issue #3: Agent Profile Timeout

**Severity:** 🟠 High  
**Location:** `crates/rot-core/src/agent.rs:62-80`  
**Symptom:** Agent selection with `--agent` flag times out

**Root Cause:**
- Agent profiles may have different system prompts
- Some agents (e.g., "plan") may require different tool sets
- Tool initialization may fail silently

**Fix Required:**
1. Add agent-specific tool validation
2. Add timeout error messages
3. Validate agent exists before execution

---

### Issue #4: No Timeout Error Messages

**Severity:** 🟠 High  
**Location:** Multiple files

**Current Behavior:**
```rust
// No informative error on timeout
Err(e) => {
    return Err(anyhow::anyhow!("exec failed"));
}
```

**Fix Required:**
1. Detect timeout vs other errors
2. Provide actionable error messages
3. Suggest remediation steps

---

### Issue #5: Interactive Features in Exec Mode

**Severity:** 🟡 Medium  
**Location:** Documentation & CLI help

**Issue:** Features that require interactive mode aren't documented:
- Slash commands (`/agents`, `/rlm`, `/help`)
- Agent switching (`@agent` syntax)
- Copy to clipboard (`y` key)
- Approval workflow

**Fix Required:**
1. Add documentation for exec vs interactive modes
2. Add `--help-features` flag to list capabilities
3. Validate incompatible flag combinations

---

### Issue #6: MCP Server Testing

**Severity:** 🟡 Medium  
**Location:** `crates/rot-mcp/`

**Issue:** No MCP servers configured by default, cannot test MCP features.

**Fix Required:**
1. Add mock MCP server for testing
2. Add MCP setup documentation
3. Add `--mcp-test` flag with built-in mock

---

### Issue #7: Minor Display Glitches

**Severity:** 🟢 Low  
**Location:** Output formatting

**Issues:**
- Occasional typos in output
- Inconsistent formatting
- Missing line breaks

**Fix Required:**
1. Review all user-facing strings
2. Add output formatting tests
3. Implement consistent formatting standards

---

### Issue #8: Approval Workflow in Exec Mode

**Severity:** 🟠 High  
**Location:** `crates/rot-core/src/permission.rs`

**Issue:** Approval workflow blocks in exec mode with no way to proceed.

**Fix Required:**
1. Add `--auto-approve` flag
2. Add `--approve-list` for specific tools
3. Add timeout with clear error message

---

## Part 2: Implementation Plan

### Phase 1: Critical Fixes (Week 1)

#### Fix 1.1: Add Timeout Handling to Task Tool

**File:** `crates/rot-tools/src/builtin/task.rs`

```rust
// Add timeout wrapper
pub async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
    let params: TaskParams = serde_json::from_value(params)?;
    
    // Add timeout
    let timeout_duration = ctx.task_policy.task_timeout;
    
    let result = tokio::time::timeout(
        timeout_duration,
        self.execute_internal(params, ctx)
    ).await;
    
    match result {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(ToolError::Timeout(format!(
            "Task execution timed out after {:?}",
            timeout_duration
        ))),
    }
}
```

**Test Case:** `test_task_timeout_returns_informative_error`

---

#### Fix 1.2: Add Auto-Approve Flag

**File:** `crates/rot-cli/src/cli.rs`

```rust
pub struct ExecArgs {
    // ... existing args
    #[arg(long, help = "Automatically approve all tool calls")]
    pub auto_approve: bool,
    
    #[arg(long, help = "Comma-separated list of tools to auto-approve")]
    pub approve_list: Option<String>,
}
```

**File:** `crates/rot-cli/src/commands/exec.rs`

```rust
let agent = Arc::new(
    Agent::new(provider, tools, config, runtime_security.clone())
        .with_session_id(target_session_id.clone())
        .with_approval_callback(if auto_approve {
            Some(Arc::new(|_tool, _args| {
                Box::pin(async { ApprovalResponse::AllowAlways })
            }))
        } else if let Some(list) = approve_list {
            let approved: HashSet<String> = list.split(',')
                .map(|s| s.trim().to_string())
                .collect();
            Some(Arc::new(move |tool, _args| {
                let approved = approved.clone();
                let tool = tool.to_string();
                Box::pin(async move {
                    if approved.contains(&tool) {
                        ApprovalResponse::AllowAlways
                    } else {
                        ApprovalResponse::DenyOnce
                    }
                })
            }))
        } else {
            None
        })
);
```

**Test Case:** `test_exec_auto_approve_flag`

---

#### Fix 1.3: Add RLM Progress Callbacks

**File:** `crates/rot-cli/src/commands/exec.rs`

```rust
let progress_callback = if options.json {
    Some(Arc::new(|msg: String| {
        eprintln!("{}", serde_json::json!({
            "type": "progress",
            "message": msg,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }).to_string());
    }))
} else {
    Some(Arc::new(|msg: String| {
        eprintln!("[RLM] {}", msg);
    }))
};

let config = rot_rlm::RlmConfig {
    on_progress: progress_callback,
    runtime_security: runtime_security.clone(),
    runtime: rlm_runtime.unwrap_or_default(),
    isolation: rlm_isolation.unwrap_or_default(),
    docker_image: rlm_docker_image,
    ..Default::default()
};
```

**Test Case:** `test_rlm_progress_reporting`

---

### Phase 2: High Priority Fixes (Week 2)

#### Fix 2.1: Improve Error Messages

**File:** `crates/rot-cli/src/commands/exec.rs`

```rust
match agent.process(&mut messages, prompt).await {
    Ok(resp) => resp,
    Err(rot_core::AgentError::Timeout(duration)) => {
        let msg = format!(
            "Operation timed out after {:.1}s.\n\
             Suggestions:\n\
             - Increase timeout with --timeout <seconds>\n\
             - Simplify your request\n\
             - Use --auto-approve for non-interactive mode",
            duration.as_secs_f64()
        );
        return Err(anyhow::anyhow!(msg));
    }
    Err(rot_core::AgentError::ApprovalRequired(tool)) => {
        let msg = format!(
            "Tool '{}' requires approval but running in non-interactive mode.\n\
             Suggestions:\n\
             - Use --auto-approve to allow all tool calls\n\
             - Use --approve-list {} to allow this specific tool\n\
             - Run in interactive mode (without 'exec')",
            tool, tool
        );
        return Err(anyhow::anyhow!(msg));
    }
    Err(e) => {
        return Err(anyhow::anyhow!("Execution failed: {}\n\
            Run with --verbose for detailed logs", e));
    }
}
```

**Test Case:** `test_informative_timeout_error`

---

#### Fix 2.2: Add Mode Documentation

**File:** Create `docs/MODES.md`

```markdown
# rot Execution Modes

## Interactive Mode (Default)
Run without subcommand to enter interactive TUI:
```bash
rot
```

**Features:**
- Slash commands (/help, /agents, /rlm, etc.)
- Agent switching (@agent syntax)
- Copy to clipboard (y key)
- Interactive approval workflow
- Real-time streaming

## Non-Interactive Mode (exec)
Run single commands with `exec`:
```bash
rot exec "your prompt"
```

**Features:**
- Single execution
- JSON output (--json)
- Auto-approval (--auto-approve)
- Session resumption (--session ID)

**Limitations:**
- No slash commands
- No @agent syntax
- No interactive approval (use --auto-approve)
```

**Test Case:** `test_mode_documentation`

---

#### Fix 2.3: Agent Validation

**File:** `crates/rot-core/src/agent.rs`

```rust
impl AgentRegistry {
    pub fn validate(name: &str) -> Result<AgentProfile, AgentError> {
        let profile = Self::get(name)
            .ok_or_else(|| AgentError::UnknownAgent(name.to_string()))?;
        
        // Validate tools are available
        // Validate system prompt
        // Check compatibility
        
        Ok(profile)
    }
}

pub enum AgentError {
    UnknownAgent(String),
    IncompatibleTools(Vec<String>),
    InvalidPrompt(String),
}
```

**Test Case:** `test_agent_validation`

---

### Phase 3: Medium Priority (Week 3)

#### Fix 3.1: Mock MCP Server

**File:** Create `crates/rot-mcp/src/mock_server.rs`

```rust
pub struct MockMcpServer {
    tools: Vec<ToolDefinition>,
}

impl MockMcpServer {
    pub fn new() -> Self {
        Self {
            tools: vec![
                ToolDefinition {
                    name: "mcp__mock_echo".to_string(),
                    description: "Echo back the input".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "message": {"type": "string"}
                        }
                    }),
                },
                ToolDefinition {
                    name: "mcp__mock_delay".to_string(),
                    description: "Wait for specified seconds".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "seconds": {"type": "number"}
                        }
                    }),
                },
            ],
        }
    }
    
    pub async fn execute(&self, tool: &str, args: Value) -> Result<Value, McpError> {
        match tool {
            "mcp__mock_echo" => {
                Ok(json!({"echo": args["message"]}))
            }
            "mcp__mock_delay" => {
                let secs = args["seconds"].as_f64().unwrap_or(1.0);
                tokio::time::sleep(Duration::from_secs_f64(secs)).await;
                Ok(json!({"delayed": secs}))
            }
            _ => Err(McpError::UnknownTool(tool.to_string())),
        }
    }
}
```

**Test Case:** `test_mock_mcp_server`

---

#### Fix 3.2: Output Formatting Standards

**File:** Create `crates/rot-cli/src/output.rs`

```rust
pub struct OutputFormatter {
    mode: OutputMode,
}

impl OutputFormatter {
    pub fn format_tool_call(&self, name: &str, args: &Value) -> String {
        match self.mode {
            OutputMode::Human => {
                format!("→ Calling tool: {}({})", name, 
                    serde_json::to_string_pretty(args).unwrap_or_default())
            }
            OutputMode::Json => {
                serde_json::to_string(&json!({
                    "type": "tool_call",
                    "name": name,
                    "arguments": args,
                })).unwrap_or_default()
            }
        }
    }
    
    pub fn format_error(&self, error: &str) -> String {
        match self.mode {
            OutputMode::Human => {
                format!("❌ Error: {}", error)
            }
            OutputMode::Json => {
                serde_json::to_string(&json!({
                    "type": "error",
                    "message": error,
                })).unwrap_or_default()
            }
        }
    }
}
```

**Test Case:** `test_output_formatting`

---

## Part 3: Comprehensive Test Strategy

### Test Pyramid

```
        ┌─────────┐
        │   E2E   │  (10 tests)
        │  Tests  │
        ├─────────┤
        │Integration│ (30 tests)
        │  Tests    │
        ├─────────┤
        │  Unit   │  (100+ tests)
        │  Tests  │
        └─────────┘
```

### Test Categories

#### A. Unit Tests (100+ tests)

##### A.1 Core Agent Tests

**File:** `crates/rot-core/tests/agent_unit_tests.rs`

```rust
#[cfg(test)]
mod agent_tests {
    use super::*;
    
    // === Message Processing ===
    
    #[test]
    fn test_message_creation_user() {
        let msg = Message::new(Role::User, "Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 1);
    }
    
    #[test]
    fn test_message_creation_assistant() {
        let msg = Message::new(Role::Assistant, "Hi there");
        assert_eq!(msg.role, Role::Assistant);
    }
    
    #[test]
    fn test_message_with_multiple_blocks() {
        let mut msg = Message::new(Role::Assistant, "Text");
        msg.add_tool_call("read", json!({"path": "/tmp/test"}));
        assert_eq!(msg.content.len(), 2);
    }
    
    // === Agent Configuration ===
    
    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_iterations, 50);
        assert_eq!(config.agent_name, "default");
    }
    
    #[test]
    fn test_agent_config_custom() {
        let config = AgentConfig {
            max_iterations: 10,
            agent_name: "custom".to_string(),
            system_prompt: Some("Be helpful".to_string()),
            ..Default::default()
        };
        assert_eq!(config.max_iterations, 10);
    }
    
    #[test]
    fn test_task_policy_default() {
        let policy = TaskExecutionPolicy::default();
        assert_eq!(policy.max_depth, 1);
        assert_eq!(policy.max_total_tasks, 8);
        assert_eq!(policy.max_concurrent_tasks, 1);
    }
    
    // === Agent Registry ===
    
    #[test]
    fn test_registry_default_agent() {
        let profile = AgentRegistry::default_agent();
        assert_eq!(profile.name, "default");
        assert!(!profile.system_prompt.is_empty());
    }
    
    #[test]
    fn test_registry_get_existing_agent() {
        let profile = AgentRegistry::get("plan");
        assert!(profile.is_some());
        assert_eq!(profile.unwrap().name, "plan");
    }
    
    #[test]
    fn test_registry_get_nonexistent_agent() {
        let profile = AgentRegistry::get("nonexistent");
        assert!(profile.is_none());
    }
    
    #[test]
    fn test_registry_all_builtin_agents() {
        let agents = AgentRegistry::builtins();
        assert!(!agents.is_empty());
        assert!(agents.iter().any(|a| a.name == "default"));
        assert!(agents.iter().any(|a| a.name == "plan"));
    }
    
    #[test]
    fn test_agent_is_subagent() {
        let default = AgentRegistry::get("default").unwrap();
        assert!(!default.is_subagent());
        
        let review = AgentRegistry::get("review").unwrap();
        assert!(review.is_subagent());
    }
    
    // === Error Handling ===
    
    #[test]
    fn test_agent_error_timeout() {
        let err = AgentError::Timeout(Duration::from_secs(30));
        let msg = err.to_string();
        assert!(msg.contains("30"));
    }
    
    #[test]
    fn test_agent_error_max_iterations() {
        let err = AgentError::MaxIterationsReached(10);
        let msg = err.to_string();
        assert!(msg.contains("10"));
    }
}
```

##### A.2 Tool Tests

**File:** `crates/rot-tools/tests/tool_unit_tests.rs`

```rust
#[cfg(test)]
mod tool_tests {
    
    // === Tool Registry ===
    
    #[test]
    fn test_registry_new() {
        let registry = ToolRegistry::new();
        assert!(registry.names().is_empty());
    }
    
    #[test]
    fn test_registry_register() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(ReadTool::new()));
        assert!(registry.get("read").is_some());
    }
    
    #[test]
    fn test_registry_get_nonexistent() {
        let registry = ToolRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }
    
    #[test]
    fn test_registry_names_sorted() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(BashTool::new()));
        registry.register(Box::new(ReadTool::new()));
        let names = registry.names();
        assert_eq!(names[0], "bash");
        assert_eq!(names[1], "read");
    }
    
    // === Read Tool ===
    
    #[test]
    fn test_read_tool_name() {
        let tool = ReadTool::new();
        assert_eq!(tool.name(), "read");
    }
    
    #[test]
    fn test_read_tool_description() {
        let tool = ReadTool::new();
        assert!(!tool.description().is_empty());
    }
    
    #[test]
    fn test_read_tool_parameters_schema() {
        let tool = ReadTool::new();
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["path"].is_object());
    }
    
    #[tokio::test]
    async fn test_read_tool_missing_path() {
        let tool = ReadTool::new();
        let result = tool.execute(json!({}), &ToolContext::default()).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_read_tool_nonexistent_file() {
        let tool = ReadTool::new();
        let result = tool.execute(
            json!({"path": "/nonexistent/file.txt"}),
            &ToolContext::default()
        ).await;
        assert!(result.is_err());
    }
    
    // === Write Tool ===
    
    #[test]
    fn test_write_tool_name() {
        let tool = WriteTool::new();
        assert_eq!(tool.name(), "write");
    }
    
    #[tokio::test]
    async fn test_write_tool_missing_path() {
        let tool = WriteTool::new();
        let result = tool.execute(
            json!({"content": "test"}),
            &ToolContext::default()
        ).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_write_tool_missing_content() {
        let tool = WriteTool::new();
        let result = tool.execute(
            json!({"path": "/tmp/test.txt"}),
            &ToolContext::default()
        ).await;
        assert!(result.is_err());
    }
    
    // === Bash Tool ===
    
    #[test]
    fn test_bash_tool_name() {
        let tool = BashTool::new();
        assert_eq!(tool.name(), "bash");
    }
    
    #[tokio::test]
    async fn test_bash_tool_missing_command() {
        let tool = BashTool::new();
        let result = tool.execute(json!({}), &ToolContext::default()).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_bash_tool_simple_command() {
        let tool = BashTool::new();
        let result = tool.execute(
            json!({"command": "echo hello"}),
            &ToolContext::default()
        ).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("hello"));
    }
    
    // === Edit Tool ===
    
    #[test]
    fn test_edit_tool_name() {
        let tool = EditTool::new();
        assert_eq!(tool.name(), "edit");
    }
    
    #[tokio::test]
    async fn test_edit_tool_missing_old_string() {
        let tool = EditTool::new();
        let result = tool.execute(
            json!({"path": "/tmp/test.txt", "newString": "new"}),
            &ToolContext::default()
        ).await;
        assert!(result.is_err());
    }
    
    // === Glob Tool ===
    
    #[test]
    fn test_glob_tool_name() {
        let tool = GlobTool::new();
        assert_eq!(tool.name(), "glob");
    }
    
    #[tokio::test]
    async fn test_glob_tool_simple_pattern() {
        let tool = GlobTool::new();
        let result = tool.execute(
            json!({"pattern": "*.rs"}),
            &ToolContext::default()
        ).await;
        assert!(result.is_ok());
    }
    
    // === Grep Tool ===
    
    #[test]
    fn test_grep_tool_name() {
        let tool = GrepTool::new();
        assert_eq!(tool.name(), "grep");
    }
    
    #[tokio::test]
    async fn test_grep_tool_simple_pattern() {
        let tool = GrepTool::new();
        let result = tool.execute(
            json!({"pattern": "fn main"}),
            &ToolContext::default()
        ).await;
        assert!(result.is_ok());
    }
    
    // === Task Tool ===
    
    #[test]
    fn test_task_tool_name() {
        let tool = TaskTool::new();
        assert_eq!(tool.name(), "task");
    }
    
    #[test]
    fn test_task_params_validation() {
        let params: TaskParams = serde_json::from_value(json!({
            "prompt": "test task"
        })).unwrap();
        assert_eq!(params.prompt, "test task");
        assert!(params.agent.is_none());
    }
    
    #[test]
    fn test_task_params_with_agent() {
        let params: TaskParams = serde_json::from_value(json!({
            "prompt": "test task",
            "agent": "review"
        })).unwrap();
        assert_eq!(params.agent, Some("review".to_string()));
    }
}
```

##### A.3 Provider Tests

**File:** `crates/rot-provider/tests/provider_unit_tests.rs`

```rust
#[cfg(test)]
mod provider_tests {
    
    // === Request Building ===
    
    #[test]
    fn test_request_new() {
        let req = Request::new();
        assert!(req.messages.is_empty());
    }
    
    #[test]
    fn test_request_with_message() {
        let mut req = Request::new();
        req.add_message(Role::User, "Hello");
        assert_eq!(req.messages.len(), 1);
    }
    
    #[test]
    fn test_request_with_system_prompt() {
        let mut req = Request::new();
        req.set_system_prompt("Be helpful");
        assert_eq!(req.system_prompt, Some("Be helpful".to_string()));
    }
    
    #[test]
    fn test_request_with_tools() {
        let mut req = Request::new();
        req.add_tool(ToolDefinition {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            parameters: json!({}),
        });
        assert_eq!(req.tools.len(), 1);
    }
    
    // === Stream Events ===
    
    #[test]
    fn test_stream_event_text_delta() {
        let event = StreamEvent::TextDelta { delta: "Hello".to_string() };
        match event {
            StreamEvent::TextDelta { delta } => assert_eq!(delta, "Hello"),
            _ => panic!("Wrong event type"),
        }
    }
    
    #[test]
    fn test_stream_event_tool_call() {
        let event = StreamEvent::ToolCall {
            id: "123".to_string(),
            name: "read".to_string(),
            arguments: json!({"path": "/tmp"}),
        };
        match event {
            StreamEvent::ToolCall { name, .. } => assert_eq!(name, "read"),
            _ => panic!("Wrong event type"),
        }
    }
    
    // === Response Parsing ===
    
    #[test]
    fn test_response_extraction() {
        let response = Response {
            content: vec![
                ContentBlock::Text { text: "Hello".to_string() },
                ContentBlock::ToolCall {
                    id: "1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({}),
                },
            ],
            stop_reason: StopReason::EndTurn,
        };
        
        let text: String = response.content.iter()
            .filter_map(|c| match c {
                ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect();
        
        assert_eq!(text, "Hello");
    }
}
```

##### A.4 Permission System Tests

**File:** `crates/rot-core/tests/permission_unit_tests.rs`

```rust
#[cfg(test)]
mod permission_tests {
    
    // === Approval Response ===
    
    #[test]
    fn test_approval_response_allow_once() {
        let response = ApprovalResponse::AllowOnce;
        assert!(response.is_allowed());
        assert!(!response.is_permanent());
    }
    
    #[test]
    fn test_approval_response_allow_always() {
        let response = ApprovalResponse::AllowAlways;
        assert!(response.is_allowed());
        assert!(response.is_permanent());
    }
    
    #[test]
    fn test_approval_response_deny_once() {
        let response = ApprovalResponse::DenyOnce;
        assert!(!response.is_allowed());
        assert!(!response.is_permanent());
    }
    
    #[test]
    fn test_approval_response_deny_always() {
        let response = ApprovalResponse::DenyAlways;
        assert!(!response.is_allowed());
        assert!(response.is_permanent());
    }
    
    // === Permission System ===
    
    #[test]
    fn test_permission_system_new() {
        let system = PermissionSystem::new(ApprovalPolicy::PerTool);
        assert_eq!(system.policy, ApprovalPolicy::PerTool);
    }
    
    #[test]
    fn test_permission_system_never_approve() {
        let system = PermissionSystem::new(ApprovalPolicy::Never);
        let result = system.check("read", json!({}));
        assert!(result.is_allowed());
    }
    
    #[test]
    fn test_permission_system_always_approve() {
        let system = PermissionSystem::new(ApprovalPolicy::Always);
        let result = system.check("read", json!({}));
        assert!(result.is_allowed());
    }
    
    // === Approval Policy ===
    
    #[test]
    fn test_approval_policy_default() {
        let policy = ApprovalPolicy::default();
        assert_eq!(policy, ApprovalPolicy::PerTool);
    }
}
```

##### A.5 Security Tests

**File:** `crates/rot-sandbox/tests/security_unit_tests.rs`

```rust
#[cfg(test)]
mod security_tests {
    
    // === Sandbox Mode ===
    
    #[test]
    fn test_sandbox_mode_default() {
        let mode = SandboxMode::default();
        assert_eq!(mode, SandboxMode::WorkspaceOnly);
    }
    
    #[test]
    fn test_sandbox_mode_restrictions() {
        let mode = SandboxMode::WorkspaceOnly;
        assert!(mode.is_restricted());
    }
    
    #[test]
    fn test_sandbox_mode_full_access() {
        let mode = SandboxMode::DangerFullAccess;
        assert!(!mode.is_restricted());
    }
    
    // === Path Validation ===
    
    #[test]
    fn test_path_validation_in_workspace() {
        let workspace = PathBuf::from("/home/user/project");
        let path = PathBuf::from("/home/user/project/src/main.rs");
        assert!(is_path_allowed(&path, &workspace));
    }
    
    #[test]
    fn test_path_validation_outside_workspace() {
        let workspace = PathBuf::from("/home/user/project");
        let path = PathBuf::from("/etc/passwd");
        assert!(!is_path_allowed(&path, &workspace));
    }
    
    #[test]
    fn test_path_validation_traversal_attack() {
        let workspace = PathBuf::from("/home/user/project");
        let path = PathBuf::from("/home/user/project/../etc/passwd");
        assert!(!is_path_allowed(&path, &workspace));
    }
    
    #[test]
    fn test_path_validation_symlink_outside() {
        // This would need actual filesystem setup
        // Test that symlinks pointing outside workspace are blocked
    }
    
    // === Runtime Security Config ===
    
    #[test]
    fn test_runtime_security_default() {
        let config = RuntimeSecurityConfig::default();
        assert_eq!(config.sandbox_mode, SandboxMode::WorkspaceOnly);
        assert_eq!(config.approval_policy, ApprovalPolicy::PerTool);
    }
    
    #[test]
    fn test_runtime_security_full_access() {
        let config = RuntimeSecurityConfig {
            sandbox_mode: SandboxMode::DangerFullAccess,
            approval_policy: ApprovalPolicy::Never,
            ..Default::default()
        };
        assert!(!config.sandbox_mode.is_restricted());
    }
}
```

---

#### B. Integration Tests (30 tests)

##### B.1 Tool Execution Integration

**File:** `tests/integration/tool_integration_tests.rs`

```rust
#[cfg(test)]
mod tool_integration {
    
    // === Read + Edit Integration ===
    
    #[tokio::test]
    async fn test_read_then_edit_workflow() {
        // Setup: Create test file
        // Execute: Read file
        // Execute: Edit file
        // Verify: Changes applied
    }
    
    #[tokio::test]
    async fn test_edit_with_line_numbers() {
        // Test editing specific lines
    }
    
    // === Glob + Read Integration ===
    
    #[tokio::test]
    async fn test_glob_then_read_multiple() {
        // Find all .rs files
        // Read each one
        // Verify all read successfully
    }
    
    // === Grep + Read Integration ===
    
    #[tokio::test]
    async fn test_grep_then_read_found_files() {
        // Search for pattern
        // Read files containing pattern
        // Verify pattern exists
    }
    
    // === Bash + Write Integration ===
    
    #[tokio::test]
    async fn test_bash_command_then_write_output() {
        // Run bash command
        // Write output to file
        // Verify file contains output
    }
    
    // === Task Tool Integration ===
    
    #[tokio::test]
    async fn test_task_delegation_simple() {
        // Create mock task runner
        // Execute task tool
        // Verify delegation occurred
    }
    
    #[tokio::test]
    async fn test_task_with_depth_limit() {
        // Test that depth limit is enforced
    }
    
    #[tokio::test]
    async fn test_task_concurrent_limit() {
        // Test concurrent task limit
    }
}
```

##### B.2 Agent Workflow Integration

**File:** `tests/integration/agent_integration_tests.rs`

```rust
#[cfg(test)]
mod agent_integration {
    
    #[tokio::test]
    async fn test_agent_simple_query() {
        // Create agent with mock provider
        // Send simple query
        // Verify response
    }
    
    #[tokio::test]
    async fn test_agent_with_tool_call() {
        // Create agent with tools
        // Send query requiring tool
        // Verify tool was called
        // Verify response includes tool result
    }
    
    #[tokio::test]
    async fn test_agent_multi_tool_workflow() {
        // Query requiring multiple tools
        // Verify all tools called in sequence
    }
    
    #[tokio::test]
    async fn test_agent_max_iterations() {
        // Set low max iterations
        // Send query that loops
        // Verify stops at max
    }
    
    #[tokio::test]
    async fn test_agent_with_approval() {
        // Test approval workflow
        // Verify tool waits for approval
    }
    
    #[tokio::test]
    async fn test_agent_approval_deny() {
        // Test denying approval
        // Verify tool not executed
    }
}
```

##### B.3 Session Integration

**File:** `tests/integration/session_integration_tests.rs`

```rust
#[cfg(test)]
mod session_integration {
    
    #[tokio::test]
    async fn test_session_create() {
        // Create new session
        // Verify ID generated
        // Verify file created
    }
    
    #[tokio::test]
    async fn test_session_append() {
        // Create session
        // Append messages
        // Verify messages saved
    }
    
    #[tokio::test]
    async fn test_session_resume() {
        // Create and populate session
        // Load session
        // Verify messages restored
    }
    
    #[tokio::test]
    async fn test_session_fork() {
        // Create parent session
        // Fork to child
        // Verify parent messages in child
        // Verify child has new ID
    }
    
    #[tokio::test]
    async fn test_session_export_import() {
        // Create session
        // Export to JSON
        // Import to new session
        // Verify identical
    }
    
    #[tokio::test]
    async fn test_session_tree() {
        // Create parent
        // Create multiple children
        // Verify tree structure
    }
}
```

##### B.4 RLM Integration

**File:** `tests/integration/rlm_integration_tests.rs`

```rust
#[cfg(test)]
mod rlm_integration {
    
    #[tokio::test]
    async fn test_rlm_simple_iteration() {
        // Create RLM engine with mock
        // Execute simple task
        // Verify iteration occurred
    }
    
    #[tokio::test]
    async fn test_rlm_with_context_file() {
        // Create context file
        // Run RLM with context
        // Verify context used
    }
    
    #[tokio::test]
    async fn test_rlm_output_schema() {
        // Define output schema
        // Run RLM
        // Verify output matches schema
    }
    
    #[tokio::test]
    async fn test_rlm_trajectory_saved() {
        // Run RLM
        // Verify trajectory file created
        // Verify contains all steps
    }
}
```

---

#### C. E2E Tests (10 tests)

##### C.1 Complete User Workflows

**File:** `tests/e2e/user_workflows.rs`

```rust
#[cfg(test)]
mod e2e_workflows {
    
    #[test]
    fn test_e2e_simple_code_review() {
        // Given: A Rust file with issues
        // When: User asks "Review this code"
        // Then: Agent identifies issues and suggests fixes
    }
    
    #[test]
    fn test_e2e_refactor_function() {
        // Given: A function that needs refactoring
        // When: User asks "Refactor this function"
        // Then: Agent reads, analyzes, and rewrites function
    }
    
    #[test]
    fn test_e2e_add_feature() {
        // Given: A codebase
        // When: User asks "Add error handling"
        // Then: Agent adds proper error handling
    }
    
    #[test]
    fn test_e2e_find_and_fix_bug() {
        // Given: Code with a bug
        // When: User describes bug symptoms
        // Then: Agent finds root cause and fixes
    }
    
    #[test]
    fn test_e2e_document_code() {
        // Given: Undocumented code
        // When: User asks "Add documentation"
        // Then: Agent adds doc comments
    }
    
    #[test]
    fn test_e2e_write_tests() {
        // Given: A function without tests
        // When: User asks "Write tests for this"
        // Then: Agent generates comprehensive tests
    }
    
    #[test]
    fn test_e2e_multi_file_refactor() {
        // Given: Code spanning multiple files
        // When: User asks to refactor across files
        // Then: Agent coordinates changes across files
    }
    
    #[test]
    fn test_e2e_explore_codebase() {
        // Given: Unfamiliar codebase
        // When: User asks "How does X work?"
        // Then: Agent explores and explains
    }
}
```

##### C.2 Error Recovery

**File:** `tests/e2e/error_recovery.rs`

```rust
#[cfg(test)]
mod e2e_errors {
    
    #[test]
    fn test_e2e_invalid_tool_arguments() {
        // When: Agent calls tool with bad args
        // Then: Error is handled gracefully
        // And: Agent retries with correct args
    }
    
    #[test]
    fn test_e2e_file_not_found() {
        // When: Trying to read nonexistent file
        // Then: Clear error message
        // And: Agent suggests alternatives
    }
    
    #[test]
    fn test_e2e_permission_denied() {
        // When: Operation requires approval
        // Then: Approval workflow triggers
        // And: Operation proceeds after approval
    }
    
    #[test]
    fn test_e2e_network_error() {
        // When: Provider API fails
        // Then: Clear error message
        // And: Retry or graceful degradation
    }
}
```

---

### Test Implementation Plan

#### Week 1: Unit Tests
- Day 1-2: Core agent tests (20 tests)
- Day 3: Tool tests (40 tests)
- Day 4: Provider tests (20 tests)
- Day 5: Security tests (20 tests)

#### Week 2: Integration Tests
- Day 1-2: Tool integration (10 tests)
- Day 3: Agent integration (10 tests)
- Day 4: Session integration (5 tests)
- Day 5: RLM integration (5 tests)

#### Week 3: E2E Tests
- Day 1-3: User workflows (8 tests)
- Day 4-5: Error recovery (2 tests)

---

## Part 4: Test Infrastructure

### Test Utilities

**File:** `tests/utils/mod.rs`

```rust
pub mod mock_provider;
pub mod mock_tool;
pub mod fixtures;
pub mod assertions;

pub use mock_provider::MockProvider;
pub use mock_tool::MockTool;

pub fn setup_test_env() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temp dir")
}

pub fn create_test_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("Failed to write test file");
    path
}
```

### Mock Provider

**File:** `tests/utils/mock_provider.rs`

```rust
pub struct MockProvider {
    responses: VecDeque<String>,
    tool_calls: Vec<(String, Value)>,
}

impl MockProvider {
    pub fn new(responses: Vec<&str>) -> Self {
        Self {
            responses: responses.into_iter().map(|s| s.to_string()).collect(),
            tool_calls: Vec::new(),
        }
    }
    
    pub fn tool_calls(&self) -> &[(String, Value)] {
        &self.tool_calls
    }
}

#[async_trait]
impl Provider for MockProvider {
    async fn complete(&self, _request: Request) -> Result<Response, ProviderError> {
        // Return next mock response
    }
    
    async fn stream(&self, _request: Request) -> Result<...> {
        // Stream mock response
    }
}
```

### CI Configuration

**File:** `.github/workflows/test.yml`

```yaml
name: Tests

on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run unit tests
        run: cargo test --lib --all
      
  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - name: Run integration tests
        run: cargo test --test '*' --all
      
  e2e-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - name: Run E2E tests
        run: cargo test --test e2e
        env:
          ZAI_API_KEY: ${{ secrets.ZAI_API_KEY }}
```

---

## Part 5: Success Metrics

### Test Coverage Goals

| Category | Current | Target | Metric |
|----------|---------|--------|--------|
| Unit Tests | ~40 | 100+ | Count |
| Integration Tests | ~10 | 30 | Count |
| E2E Tests | 0 | 10 | Count |
| Code Coverage | ~60% | 85% | % |
| Critical Path Coverage | ~70% | 100% | % |

### Quality Gates

**Before Merge:**
- ✅ All unit tests pass
- ✅ All integration tests pass
- ✅ No clippy warnings
- ✅ Code formatted
- ✅ Coverage ≥ 80%

**Before Release:**
- ✅ All E2E tests pass
- ✅ Manual QA completed
- ✅ Performance benchmarks pass
- ✅ Security audit passed
- ✅ Documentation updated

---

## Appendix A: Test Case Templates

### Unit Test Template

```rust
#[test]
fn test_<feature>_<scenario>() {
    // Arrange
    let input = ...;
    let expected = ...;
    
    // Act
    let result = function_under_test(input);
    
    // Assert
    assert_eq!(result, expected);
}
```

### Integration Test Template

```rust
#[tokio::test]
async fn test_<feature>_<scenario>() {
    // Setup
    let temp_dir = setup_test_env();
    let provider = MockProvider::new(vec!["response"]);
    
    // Execute
    let result = perform_action().await;
    
    // Verify
    assert!(result.is_ok());
    
    // Cleanup (automatic with temp_dir)
}
```

### E2E Test Template

```rust
#[test]
fn test_e2e_<workflow>() {
    // Given
    let binary = env!("CARGO_BIN_EXE_rot");
    let temp_dir = setup_test_env();
    
    // When
    let output = Command::new(binary)
        .args(&["exec", "prompt"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute");
    
    // Then
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("expected"));
}
```

---

## Appendix B: Running Tests

### All Tests
```bash
cargo test --all
```

### Specific Category
```bash
# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test '*'

# E2E tests only
cargo test --test e2e
```

### With Coverage
```bash
cargo tarpaulin --all --out Html
```

### Specific Test
```bash
cargo test test_read_tool_simple_command
```

### Watch Mode
```bash
cargo watch -x test
```

---

## Conclusion

This comprehensive plan addresses:

1. **Root Cause Analysis:** 8 issues identified and analyzed
2. **Implementation Plan:** 3-phase approach with specific fixes
3. **Test Strategy:** 140+ tests across unit/integration/E2E
4. **Timeline:** 3-week implementation schedule
5. **Quality Gates:** Clear success metrics

**Next Steps:**
1. Review and approve plan
2. Create feature branches for fixes
3. Implement Phase 1 critical fixes
4. Build out test infrastructure
5. Execute test plan

**Estimated Effort:**
- Research: 1 week ✅ (Complete)
- Implementation: 3 weeks
- Testing: 2 weeks
- **Total: 6 weeks**
