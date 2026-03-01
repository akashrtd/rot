//! Agent loop implementation.
//!
//! The agent loop is the core processing cycle:
//! 1. Send messages to the provider
//! 2. Parse the streaming response
//! 3. Execute any tool calls
//! 4. Repeat until done or max iterations reached

use crate::message::{ContentBlock, Message, Role};
use crate::permission::{ApprovalResponse, PermissionSystem};
use crate::security::{RuntimeSecurityConfig, SandboxMode};
use futures::future::join_all;
use futures::StreamExt;
use rot_session::{SessionEntry, SessionStore};
use rot_provider::{
    Provider, ProviderContent, ProviderError, ProviderMessage, Request, StopReason, StreamEvent,
    ToolDefinition,
};
use rot_tools::{TaskExecution, TaskRequest, TaskRunner, ToolContext, ToolRegistry};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const MAX_ITERATIONS: usize = 50;

/// Execution limits for delegated `task` tool calls.
#[derive(Debug, Clone)]
pub struct TaskExecutionPolicy {
    /// Maximum allowed nested task depth.
    pub max_depth: usize,
    /// Maximum number of delegated subtasks that may be started in one agent run.
    pub max_total_tasks: usize,
    /// Maximum number of delegated subtasks that may run at the same time.
    pub max_concurrent_tasks: usize,
    /// Timeout applied to each delegated subtask, including waiting for concurrency budget.
    pub task_timeout: Duration,
}

impl Default for TaskExecutionPolicy {
    fn default() -> Self {
        Self {
            max_depth: 1,
            max_total_tasks: 8,
            max_concurrent_tasks: 1,
            task_timeout: Duration::from_secs(120),
        }
    }
}

/// Agent configuration.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of tool-use iterations before stopping.
    pub max_iterations: usize,
    /// Selected agent name.
    pub agent_name: String,
    /// System prompt.
    pub system_prompt: Option<String>,
    /// Maximum tokens per response.
    pub max_tokens: Option<usize>,
    /// Limits for delegated task execution.
    pub task_policy: TaskExecutionPolicy,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: MAX_ITERATIONS,
            agent_name: "default".to_string(),
            system_prompt: None,
            max_tokens: None,
            task_policy: TaskExecutionPolicy::default(),
        }
    }
}

/// Callback for streaming events.
pub type EventCallback = Box<dyn Fn(&StreamEvent) + Send + Sync>;

/// Callback to request interactive approval from the user before running a tool.
pub type ApprovalCallback = Box<
    dyn Fn(
            &str, // Tool name
            &serde_json::Value, // Tool arguments
        ) -> Pin<Box<dyn Future<Output = ApprovalResponse> + Send>>
        + Send
        + Sync,
>;

/// The main agent that orchestrates LLM calls and tool execution.
pub struct Agent {
    provider: Box<dyn Provider>,
    tools: ToolRegistry,
    config: AgentConfig,
    runtime_security: RuntimeSecurityConfig,
    session_id: Option<String>,
    on_event: Option<EventCallback>,
    on_approval: Option<ApprovalCallback>,
    permission_system: Arc<Mutex<PermissionSystem>>,
    task_controller: Arc<TaskController>,
}

impl Agent {
    /// Create a new agent.
    pub fn new(
        provider: Box<dyn Provider>,
        tools: ToolRegistry,
        config: AgentConfig,
        runtime_security: RuntimeSecurityConfig,
    ) -> Self {
        let permission_system = PermissionSystem::new(runtime_security.approval_policy);
        let task_policy = config.task_policy.clone();
        Self {
            provider,
            tools,
            config,
            runtime_security,
            session_id: None,
            on_event: None,
            on_approval: None,
            permission_system: Arc::new(Mutex::new(permission_system)),
            task_controller: Arc::new(TaskController::new(task_policy)),
        }
    }

    /// Attach a session ID to this agent instance.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set the event callback for streaming updates.
    pub fn on_event(mut self, callback: EventCallback) -> Self {
        self.on_event = Some(callback);
        self
    }

    /// Set the approval callback for interactive permission requests.
    pub fn on_approval(mut self, callback: ApprovalCallback) -> Self {
        self.on_approval = Some(callback);
        self
    }

    /// Process user input and return the assistant's response.
    ///
    /// This runs the full agent loop: send to provider → parse response →
    /// execute tools → send tool results → repeat until done.
    pub async fn process(
        self: &Arc<Self>,
        messages: &mut Vec<Message>,
        user_input: &str,
    ) -> Result<Message, AgentProcessError> {
        let invocation = AgentInvocation {
            session_id: self.session_id.clone().unwrap_or_default(),
            system_prompt: self.config.system_prompt.clone(),
            task_depth: 0,
        };
        self.process_with_invocation(messages, user_input, invocation)
            .await
    }

    async fn process_with_invocation(
        self: &Arc<Self>,
        messages: &mut Vec<Message>,
        user_input: &str,
        invocation: AgentInvocation,
    ) -> Result<Message, AgentProcessError> {
        // Add user message
        let user_msg = Message::user(user_input);
        messages.push(user_msg);

        let working_dir = std::env::current_dir().unwrap_or_default();
        let tool_ctx = ToolContext {
            working_dir: working_dir.clone(),
            session_id: invocation.session_id.clone(),
            timeout: std::time::Duration::from_secs(120),
            sandbox_mode: match self.runtime_security.sandbox_mode {
                SandboxMode::ReadOnly => rot_tools::SandboxMode::ReadOnly,
                SandboxMode::WorkspaceWrite => rot_tools::SandboxMode::WorkspaceWrite,
                SandboxMode::DangerFullAccess => rot_tools::SandboxMode::DangerFullAccess,
            },
            network_access: self.runtime_security.sandbox_network_access
                || self.runtime_security.sandbox_mode == SandboxMode::DangerFullAccess,
            task_depth: invocation.task_depth,
            max_task_depth: self.config.task_policy.max_depth,
            task_runner: Some(Arc::new(AgentTaskRunner {
                agent: Arc::clone(self),
                parent_session_id: invocation.session_id.clone(),
                working_dir,
                task_depth: invocation.task_depth,
            })),
        };

        for _iteration in 0..self.config.max_iterations {
            // Build provider request
            let provider_messages = self.convert_messages(messages);
            let tool_defs = self.build_tool_definitions();

            let request = Request {
                messages: provider_messages,
                tools: tool_defs,
                system: invocation.system_prompt.clone(),
                max_tokens: self.config.max_tokens,
                thinking: None,
            };

            // Stream the response
            let mut stream = self
                .provider
                .stream(request)
                .await
                .map_err(AgentProcessError::Provider)?;

            let mut text_content = String::new();
            let mut tool_calls: Vec<PendingToolCall> = Vec::new();
            let mut current_tool: Option<PendingToolCall> = None;
            let mut stop_reason = StopReason::EndTurn;

            while let Some(event) = stream.next().await {
                let event = event.map_err(AgentProcessError::Provider)?;

                // Notify callback
                if let Some(ref cb) = self.on_event {
                    cb(&event);
                }

                match event {
                    StreamEvent::TextDelta { delta } => {
                        text_content.push_str(&delta);
                    }
                    StreamEvent::ToolCallStart { id, name } => {
                        current_tool = Some(PendingToolCall {
                            id,
                            name,
                            arguments: String::new(),
                        });
                    }
                    StreamEvent::ToolCallDelta { delta, .. } => {
                        if let Some(ref mut tc) = current_tool {
                            tc.arguments.push_str(&delta);
                        }
                    }
                    StreamEvent::ToolCallEnd { .. } => {
                        if let Some(tc) = current_tool.take() {
                            tool_calls.push(tc);
                        }
                    }
                    StreamEvent::Done { reason } => {
                        stop_reason = reason;
                        break;
                    }
                    _ => {}
                }
            }

            // Build assistant message
            let mut content_blocks = Vec::new();
            if !text_content.is_empty() {
                content_blocks.push(ContentBlock::Text {
                    text: text_content,
                });
            }
            for tc in &tool_calls {
                let args: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
                content_blocks.push(ContentBlock::ToolCall {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments: args,
                });
            }

            let assistant_msg = Message::assistant(content_blocks);
            messages.push(assistant_msg);

            // If no tool calls, we're done
            if tool_calls.is_empty() || stop_reason != StopReason::ToolUse {
                return Ok(messages.last().cloned().unwrap());
            }

            // Execute tool calls and add results. Task calls may run concurrently.
            let mut tool_messages: Vec<(usize, Message)> = Vec::new();
            let mut parallel_task_calls: Vec<(usize, PendingToolCall, serde_json::Value)> = Vec::new();

            for (idx, tc) in tool_calls.iter().enumerate() {
                let args: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);

                // Permission check
                let (is_denied, requires_approval) = {
                    let lock = self.permission_system.lock().unwrap();
                    (lock.is_denied(&tc.name), lock.requires_approval(&tc.name))
                };
                
                if is_denied {
                    tool_messages.push((idx, Message::tool_result(
                        tc.id.clone(),
                        format!("Execution of '{}' is permanently denied for this session.", tc.name),
                        true,
                    )));
                    continue;
                }

                if requires_approval {
                    if let Some(ref approval_cb) = self.on_approval {
                        let response = approval_cb(&tc.name, &args).await;
                        self.permission_system.lock().unwrap().handle_response(&tc.name, &response);

                        match response {
                            ApprovalResponse::DenyOnce | ApprovalResponse::DenyAlways => {
                                tool_messages.push((idx, Message::tool_result(
                                    tc.id.clone(),
                                    format!("User denied permission to run '{}'", tc.name),
                                    true,
                                )));
                                continue;
                            }
                            _ => {} // Allowed, proceed to execute
                        }
                    } else {
                        // If no callback is hooked up but approval is required, fail safe.
                        tool_messages.push((idx, Message::tool_result(
                            tc.id.clone(),
                            format!("Cannot execute '{}': No interactive approval handler configured.", tc.name),
                            true,
                        )));
                        continue;
                    }
                }

                if tc.name == "task" {
                    parallel_task_calls.push((idx, tc.clone(), args));
                } else {
                    let tool_msg = self
                        .execute_tool_call(tc.clone(), args, tool_ctx.clone())
                        .await;
                    tool_messages.push((idx, tool_msg));
                }
            }

            if !parallel_task_calls.is_empty() {
                let task_results = join_all(parallel_task_calls.into_iter().map(|(idx, tc, args)| {
                    let agent = Arc::clone(self);
                    let tool_ctx = tool_ctx.clone();
                    async move { (idx, agent.execute_tool_call(tc, args, tool_ctx).await) }
                }))
                .await;
                tool_messages.extend(task_results);
            }

            tool_messages.sort_by_key(|(idx, _)| *idx);
            for (_, tool_msg) in tool_messages {
                messages.push(tool_msg);
            }

            // Continue the loop — provider will see tool results
        }

        Err(AgentProcessError::MaxIterations(
            self.config.max_iterations,
        ))
    }

    /// Convert internal messages to provider format.
    fn convert_messages(&self, messages: &[Message]) -> Vec<ProviderMessage> {
        messages
            .iter()
            .filter(|m| m.role != Role::System) // System is handled separately
            .map(|msg| {
                let role = match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "user", // Tool results sent as user messages
                    Role::System => "system",
                };

                let content = msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => ProviderContent::Text {
                            text: text.clone(),
                        },
                        ContentBlock::ToolCall {
                            id,
                            name,
                            arguments,
                        } => ProviderContent::ToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            arguments: arguments.clone(),
                        },
                        ContentBlock::ToolResult {
                            tool_call_id,
                            content,
                            is_error,
                            ..
                        } => ProviderContent::ToolResult {
                            tool_call_id: tool_call_id.clone(),
                            content: content.clone(),
                            is_error: *is_error,
                        },
                        ContentBlock::Image { data, mime_type } => ProviderContent::Image {
                            data: data.clone(),
                            mime_type: mime_type.clone(),
                        },
                        ContentBlock::Thinking { .. } => ProviderContent::Text {
                            text: String::new(),
                        },
                    })
                    .collect();

                ProviderMessage {
                    role: role.to_string(),
                    content,
                }
            })
            .collect()
    }

    /// Build tool definitions for the provider.
    fn build_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .all()
            .iter()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters_schema(),
            })
            .collect()
    }

    async fn execute_tool_call(
        self: &Arc<Self>,
        tool_call: PendingToolCall,
        args: serde_json::Value,
        tool_ctx: ToolContext,
    ) -> Message {
        let result = if let Some(tool) = self.tools.get(&tool_call.name) {
            match tool.execute(args, &tool_ctx).await {
                Ok(result) => result,
                Err(e) => rot_tools::ToolResult::error(format!("Tool error: {e}")),
            }
        } else {
            rot_tools::ToolResult::error(format!("Unknown tool: {}", tool_call.name))
        };

        Message::tool_result_with_metadata(
            tool_call.id,
            result.output,
            result.is_error,
            result.metadata,
        )
    }
}

#[derive(Clone)]
struct AgentInvocation {
    session_id: String,
    system_prompt: Option<String>,
    task_depth: usize,
}

struct AgentTaskRunner {
    agent: Arc<Agent>,
    parent_session_id: String,
    working_dir: PathBuf,
    task_depth: usize,
}

#[async_trait::async_trait]
impl TaskRunner for AgentTaskRunner {
    async fn run_task(&self, request: TaskRequest) -> Result<TaskExecution, rot_tools::ToolError> {
        let profile = crate::AgentRegistry::get(&request.agent).ok_or_else(|| {
            rot_tools::ToolError::InvalidParameters(format!(
                "Unknown agent '{}'",
                request.agent
            ))
        })?;

        if !profile.is_subagent() {
            return Err(rot_tools::ToolError::PermissionDenied(format!(
                "Agent '{}' is not a subagent",
                profile.name
            )));
        }

        let _budget = self.agent.task_controller.acquire().await?;

        let session_store = SessionStore::new();
        let child_session = if self.parent_session_id.is_empty() {
            None
        } else {
            let child = session_store
                .create_child(
                    &self.working_dir,
                    self.agent.provider.current_model(),
                    self.agent.provider.name(),
                    &self.parent_session_id,
                    None,
                    Some(profile.name),
                )
                .await
                .map_err(|e| {
                    rot_tools::ToolError::ExecutionError(format!(
                        "Failed to create child session: {e}"
                    ))
                })?;

            session_store
                .append_by_id(
                    &self.working_dir,
                    &self.parent_session_id,
                    SessionEntry::ChildSessionLink {
                        id: ulid::Ulid::new().to_string(),
                        parent_session_id: self.parent_session_id.clone(),
                        child_session_id: child.id.clone(),
                        timestamp: current_timestamp(),
                        agent: profile.name.to_string(),
                        prompt: request.prompt.clone(),
                    },
                )
                .await
                .map_err(|e| {
                    rot_tools::ToolError::ExecutionError(format!(
                        "Failed to link child session: {e}"
                    ))
                })?;

            Some(child)
        };

        let child_session_id = child_session.as_ref().map(|session| session.id.clone());
        let invocation = AgentInvocation {
            session_id: child_session_id.clone().unwrap_or_default(),
            system_prompt: Some(profile.system_prompt.to_string()),
            task_depth: self.task_depth + 1,
        };
        let mut messages = Vec::new();
        let response = tokio::time::timeout(
            self.agent.config.task_policy.task_timeout,
            self.agent
                .process_with_invocation(&mut messages, &request.prompt, invocation),
        )
        .await
        .map_err(|_| {
            rot_tools::ToolError::Timeout(format!(
                "Subagent '{}' timed out after {:?}",
                profile.name, self.agent.config.task_policy.task_timeout
            ))
        })?
        .map_err(|e| {
            rot_tools::ToolError::ExecutionError(format!("Subagent execution failed: {e}"))
        })?;

        if let Some(mut child_session) = child_session {
            let entries = messages_to_session_entries(&messages).map_err(|e| {
                rot_tools::ToolError::ExecutionError(format!(
                    "Failed to serialize child session messages: {e}"
                ))
            })?;

            for entry in entries {
                session_store.append(&mut child_session, entry).await.map_err(|e| {
                    rot_tools::ToolError::ExecutionError(format!(
                        "Failed to persist child session transcript: {e}"
                    ))
                })?;
            }
        }

        Ok(TaskExecution {
            final_text: response.text(),
            child_session_id,
            agent: profile.name.to_string(),
        })
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn messages_to_session_entries(messages: &[Message]) -> Result<Vec<SessionEntry>, serde_json::Error> {
    let mut entries = Vec::new();

    for message in messages {
        entries.push(SessionEntry::Message {
            id: message.id.to_string(),
            parent_id: message.parent_id.as_ref().map(ToString::to_string),
            timestamp: message.timestamp,
            role: message.role.to_string(),
            content: serde_json::to_value(&message.content)?,
        });

        for (idx, block) in message.content.iter().enumerate() {
            match block {
                ContentBlock::ToolCall {
                    id,
                    name,
                    arguments,
                } => entries.push(SessionEntry::ToolCall {
                    id: id.clone(),
                    parent_id: message.id.to_string(),
                    timestamp: message.timestamp,
                    name: name.clone(),
                    arguments: arguments.clone(),
                }),
                ContentBlock::ToolResult {
                    tool_call_id,
                    content,
                    is_error,
                    ..
                } => entries.push(SessionEntry::ToolResult {
                    id: format!("{}:tool_result:{idx}", message.id),
                    call_id: tool_call_id.clone(),
                    timestamp: message.timestamp,
                    output: content.clone(),
                    is_error: *is_error,
                }),
                ContentBlock::Text { .. }
                | ContentBlock::Image { .. }
                | ContentBlock::Thinking { .. } => {}
            }
        }
    }

    Ok(entries)
}

struct TaskController {
    policy: TaskExecutionPolicy,
    total_started: AtomicUsize,
    semaphore: Arc<Semaphore>,
}

impl TaskController {
    fn new(policy: TaskExecutionPolicy) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(policy.max_concurrent_tasks.max(1))),
            policy,
            total_started: AtomicUsize::new(0),
        }
    }

    async fn acquire(&self) -> Result<TaskBudgetTicket, rot_tools::ToolError> {
        self.reserve_total_budget()?;

        let permit = tokio::time::timeout(
            self.policy.task_timeout,
            self.semaphore.clone().acquire_owned(),
        )
        .await
        .map_err(|_| {
            rot_tools::ToolError::Timeout(format!(
                "Timed out waiting for subtask concurrency slot after {:?}",
                self.policy.task_timeout
            ))
        })?
        .map_err(|_| {
            rot_tools::ToolError::ExecutionError(
                "Task controller closed unexpectedly".to_string(),
            )
        })?;

        Ok(TaskBudgetTicket { _permit: permit })
    }

    fn reserve_total_budget(&self) -> Result<(), rot_tools::ToolError> {
        loop {
            let current = self.total_started.load(Ordering::SeqCst);
            if current >= self.policy.max_total_tasks {
                return Err(rot_tools::ToolError::PermissionDenied(format!(
                    "task budget exhausted (max_total_tasks={})",
                    self.policy.max_total_tasks
                )));
            }

            if self
                .total_started
                .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Ok(());
            }
        }
    }
}

#[derive(Debug)]
struct TaskBudgetTicket {
    _permit: OwnedSemaphorePermit,
}

/// Pending tool call being accumulated from streaming events.
#[derive(Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

/// Errors that can occur during agent processing.
#[derive(Debug, thiserror::Error)]
pub enum AgentProcessError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Max iterations ({0}) reached")]
    MaxIterations(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream::{self, BoxStream, StreamExt};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::Mutex as StdMutex;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_iterations, MAX_ITERATIONS);
        assert!(config.system_prompt.is_none());
        assert_eq!(config.task_policy.max_depth, 1);
        assert_eq!(config.task_policy.max_total_tasks, 8);
    }

    #[test]
    fn test_convert_messages() {
        let agent = Agent::new(
            // We can't easily create a mock provider here without more infrastructure,
            // but we can test the conversion logic by creating a minimal test.
            // For now, just verify the config and types compile.
            Box::new(DummyProvider),
            ToolRegistry::new(),
            AgentConfig::default(),
            RuntimeSecurityConfig {
                approval_policy: crate::security::ApprovalPolicy::Never,
                ..RuntimeSecurityConfig::default()
            },
        );

        let messages = vec![
            Message::user("Hello"),
            Message::assistant(vec![ContentBlock::Text {
                text: "Hi!".to_string(),
            }]),
        ];

        let converted = agent.convert_messages(&messages);
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "user");
        assert_eq!(converted[1].role, "assistant");
    }

    #[tokio::test]
    async fn test_task_tool_delegates_to_subagent() {
        let provider = Box::new(TaskFlowProvider {
            step: StdMutex::new(0),
        });
        let mut tools = ToolRegistry::new();
        rot_tools::register_all(&mut tools);
        let agent = Arc::new(Agent::new(
            provider,
            tools,
            AgentConfig::default(),
            RuntimeSecurityConfig {
                approval_policy: crate::security::ApprovalPolicy::Never,
                ..RuntimeSecurityConfig::default()
            },
        ));

        let mut messages = Vec::new();
        let response = agent.process(&mut messages, "start").await.unwrap();
        assert_eq!(response.text(), "parent final");

        let tool_result = messages
            .iter()
            .find_map(|message| {
                message.content.iter().find_map(|block| match block {
                    ContentBlock::ToolResult {
                        content, metadata, ..
                    } => Some((content.clone(), metadata.clone())),
                    _ => None,
                })
            })
            .expect("expected task tool result");
        assert_eq!(tool_result.0, "subagent result");
        assert!(tool_result.1["child_session_id"].is_null());
    }

    #[tokio::test]
    async fn test_task_timeout_surfaces_as_tool_error() {
        let provider = Box::new(TaskTimeoutProvider {
            step: StdMutex::new(0),
        });
        let mut tools = ToolRegistry::new();
        rot_tools::register_all(&mut tools);
        let mut config = AgentConfig::default();
        config.task_policy.task_timeout = Duration::from_millis(10);

        let agent = Arc::new(Agent::new(
            provider,
            tools,
            config,
            RuntimeSecurityConfig {
                approval_policy: crate::security::ApprovalPolicy::Never,
                ..RuntimeSecurityConfig::default()
            },
        ));

        let mut messages = Vec::new();
        let response = agent.process(&mut messages, "start").await.unwrap();
        assert_eq!(response.text(), "parent recovered");

        let timeout_error = messages.iter().find_map(|message| {
            message.content.iter().find_map(|block| match block {
                ContentBlock::ToolResult {
                    content, is_error, ..
                } if *is_error => Some(content.clone()),
                _ => None,
            })
        });
        let Some(timeout_error) = timeout_error else {
            panic!("expected timeout tool result");
        };
        assert!(timeout_error.contains("timed out"));
    }

    #[tokio::test]
    async fn test_parallel_task_calls_run_concurrently() {
        let state = Arc::new(ParallelTaskState {
            call_count: AtomicUsize::new(0),
            active_subagents: AtomicUsize::new(0),
            max_active_subagents: AtomicUsize::new(0),
        });
        let provider = Box::new(ParallelTaskProvider {
            state: Arc::clone(&state),
        });
        let mut tools = ToolRegistry::new();
        rot_tools::register_all(&mut tools);
        let mut config = AgentConfig::default();
        config.task_policy.max_total_tasks = 4;
        config.task_policy.max_concurrent_tasks = 2;
        config.task_policy.task_timeout = Duration::from_secs(1);

        let agent = Arc::new(Agent::new(
            provider,
            tools,
            config,
            RuntimeSecurityConfig {
                approval_policy: crate::security::ApprovalPolicy::Never,
                ..RuntimeSecurityConfig::default()
            },
        ));

        let mut messages = Vec::new();
        let response = agent.process(&mut messages, "fan out").await.unwrap();
        assert_eq!(response.text(), "parallel done");
        assert!(state.max_active_subagents.load(AtomicOrdering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn test_task_controller_enforces_total_budget() {
        let controller = TaskController::new(TaskExecutionPolicy {
            max_total_tasks: 1,
            ..TaskExecutionPolicy::default()
        });

        let _ticket = controller.acquire().await.unwrap();
        let err = controller.acquire().await.unwrap_err();
        assert!(matches!(err, rot_tools::ToolError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn test_task_controller_times_out_on_concurrency_limit() {
        let controller = TaskController::new(TaskExecutionPolicy {
            max_concurrent_tasks: 1,
            task_timeout: Duration::from_millis(10),
            ..TaskExecutionPolicy::default()
        });

        let _first = controller.acquire().await.unwrap();
        let err = controller.acquire().await.unwrap_err();
        assert!(matches!(err, rot_tools::ToolError::Timeout(_)));
    }

    #[test]
    fn test_messages_to_session_entries_emits_tool_entries() {
        let message = Message::assistant(vec![
            ContentBlock::Text {
                text: "hello".to_string(),
            },
            ContentBlock::ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: serde_json::json!({"path":"src/main.rs"}),
            },
        ]);
        let tool_result = Message::tool_result("call-1", "ok", false);

        let entries = messages_to_session_entries(&[message, tool_result]).unwrap();
        assert!(entries.iter().any(|entry| matches!(entry, SessionEntry::Message { .. })));
        assert!(entries.iter().any(|entry| matches!(entry, SessionEntry::ToolCall { name, .. } if name == "read")));
        assert!(entries.iter().any(|entry| matches!(entry, SessionEntry::ToolResult { call_id, .. } if call_id == "call-1")));
    }

    // Minimal dummy provider for testing conversion logic
    struct DummyProvider;

    struct TaskFlowProvider {
        step: StdMutex<usize>,
    }

    struct TaskTimeoutProvider {
        step: StdMutex<usize>,
    }

    struct ParallelTaskState {
        call_count: AtomicUsize,
        active_subagents: AtomicUsize,
        max_active_subagents: AtomicUsize,
    }

    struct ParallelTaskProvider {
        state: Arc<ParallelTaskState>,
    }

    #[async_trait::async_trait]
    impl Provider for DummyProvider {
        fn name(&self) -> &str {
            "dummy"
        }
        fn models(&self) -> Vec<rot_provider::ModelInfo> {
            vec![]
        }
        fn current_model(&self) -> &str {
            "dummy"
        }
        fn set_model(&mut self, _: &str) -> Result<(), ProviderError> {
            Ok(())
        }
        async fn stream(
            &self,
            _: Request,
        ) -> Result<futures::stream::BoxStream<'_, Result<StreamEvent, ProviderError>>, ProviderError>
        {
            Ok(Box::pin(futures::stream::empty()))
        }
        async fn complete(
            &self,
            _: Request,
        ) -> Result<rot_provider::Response, ProviderError> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl Provider for TaskFlowProvider {
        fn name(&self) -> &str {
            "dummy"
        }

        fn models(&self) -> Vec<rot_provider::ModelInfo> {
            vec![]
        }

        fn current_model(&self) -> &str {
            "dummy"
        }

        fn set_model(&mut self, _: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn stream(
            &self,
            _: Request,
        ) -> Result<BoxStream<'_, Result<StreamEvent, ProviderError>>, ProviderError> {
            let step = {
                let mut lock = self.step.lock().unwrap();
                *lock += 1;
                *lock
            };

            let events = match step {
                1 => vec![
                    Ok(StreamEvent::ToolCallStart {
                        id: "task-call-1".to_string(),
                        name: "task".to_string(),
                    }),
                    Ok(StreamEvent::ToolCallDelta {
                        id: "task-call-1".to_string(),
                        delta:
                            "{\"agent\":\"review\",\"prompt\":\"inspect these changes\"}"
                                .to_string(),
                    }),
                    Ok(StreamEvent::ToolCallEnd {
                        id: "task-call-1".to_string(),
                    }),
                    Ok(StreamEvent::Done {
                        reason: StopReason::ToolUse,
                    }),
                ],
                2 => vec![
                    Ok(StreamEvent::TextDelta {
                        delta: "subagent result".to_string(),
                    }),
                    Ok(StreamEvent::Done {
                        reason: StopReason::EndTurn,
                    }),
                ],
                3 => vec![
                    Ok(StreamEvent::TextDelta {
                        delta: "parent final".to_string(),
                    }),
                    Ok(StreamEvent::Done {
                        reason: StopReason::EndTurn,
                    }),
                ],
                other => panic!("unexpected provider step {other}"),
            };

            Ok(stream::iter(events).boxed())
        }

        async fn complete(&self, _: Request) -> Result<rot_provider::Response, ProviderError> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl Provider for TaskTimeoutProvider {
        fn name(&self) -> &str {
            "dummy"
        }

        fn models(&self) -> Vec<rot_provider::ModelInfo> {
            vec![]
        }

        fn current_model(&self) -> &str {
            "dummy"
        }

        fn set_model(&mut self, _: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn stream(
            &self,
            _: Request,
        ) -> Result<BoxStream<'_, Result<StreamEvent, ProviderError>>, ProviderError> {
            let step = {
                let mut lock = self.step.lock().unwrap();
                *lock += 1;
                *lock
            };

            let stream: BoxStream<'_, Result<StreamEvent, ProviderError>> = match step {
                1 => stream::iter(vec![
                    Ok(StreamEvent::ToolCallStart {
                        id: "task-call-timeout".to_string(),
                        name: "task".to_string(),
                    }),
                    Ok(StreamEvent::ToolCallDelta {
                        id: "task-call-timeout".to_string(),
                        delta: "{\"agent\":\"review\",\"prompt\":\"stall\"}".to_string(),
                    }),
                    Ok(StreamEvent::ToolCallEnd {
                        id: "task-call-timeout".to_string(),
                    }),
                    Ok(StreamEvent::Done {
                        reason: StopReason::ToolUse,
                    }),
                ])
                .boxed(),
                2 => stream::pending().boxed(),
                3 => stream::iter(vec![
                    Ok(StreamEvent::TextDelta {
                        delta: "parent recovered".to_string(),
                    }),
                    Ok(StreamEvent::Done {
                        reason: StopReason::EndTurn,
                    }),
                ])
                .boxed(),
                other => panic!("unexpected provider step {other}"),
            };

            Ok(stream)
        }

        async fn complete(&self, _: Request) -> Result<rot_provider::Response, ProviderError> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl Provider for ParallelTaskProvider {
        fn name(&self) -> &str {
            "dummy"
        }

        fn models(&self) -> Vec<rot_provider::ModelInfo> {
            vec![]
        }

        fn current_model(&self) -> &str {
            "dummy"
        }

        fn set_model(&mut self, _: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn stream(
            &self,
            _: Request,
        ) -> Result<BoxStream<'_, Result<StreamEvent, ProviderError>>, ProviderError> {
            let call_no = self.state.call_count.fetch_add(1, AtomicOrdering::SeqCst) + 1;

            match call_no {
                1 => Ok(stream::iter(vec![
                    Ok(StreamEvent::ToolCallStart {
                        id: "task-1".to_string(),
                        name: "task".to_string(),
                    }),
                    Ok(StreamEvent::ToolCallDelta {
                        id: "task-1".to_string(),
                        delta: "{\"agent\":\"review\",\"prompt\":\"first\"}".to_string(),
                    }),
                    Ok(StreamEvent::ToolCallEnd {
                        id: "task-1".to_string(),
                    }),
                    Ok(StreamEvent::ToolCallStart {
                        id: "task-2".to_string(),
                        name: "task".to_string(),
                    }),
                    Ok(StreamEvent::ToolCallDelta {
                        id: "task-2".to_string(),
                        delta: "{\"agent\":\"explore\",\"prompt\":\"second\"}".to_string(),
                    }),
                    Ok(StreamEvent::ToolCallEnd {
                        id: "task-2".to_string(),
                    }),
                    Ok(StreamEvent::Done {
                        reason: StopReason::ToolUse,
                    }),
                ])
                .boxed()),
                2 | 3 => {
                    let active = self
                        .state
                        .active_subagents
                        .fetch_add(1, AtomicOrdering::SeqCst)
                        + 1;
                    loop {
                        let max_seen = self
                            .state
                            .max_active_subagents
                            .load(AtomicOrdering::SeqCst);
                        if active <= max_seen {
                            break;
                        }
                        if self
                            .state
                            .max_active_subagents
                            .compare_exchange(
                                max_seen,
                                active,
                                AtomicOrdering::SeqCst,
                                AtomicOrdering::SeqCst,
                            )
                            .is_ok()
                        {
                            break;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    self.state
                        .active_subagents
                        .fetch_sub(1, AtomicOrdering::SeqCst);
                    Ok(stream::iter(vec![
                        Ok(StreamEvent::TextDelta {
                            delta: format!("subagent {}", call_no),
                        }),
                        Ok(StreamEvent::Done {
                            reason: StopReason::EndTurn,
                        }),
                    ])
                    .boxed())
                }
                4 => Ok(stream::iter(vec![
                    Ok(StreamEvent::TextDelta {
                        delta: "parallel done".to_string(),
                    }),
                    Ok(StreamEvent::Done {
                        reason: StopReason::EndTurn,
                    }),
                ])
                .boxed()),
                other => panic!("unexpected provider step {other}"),
            }
        }

        async fn complete(&self, _: Request) -> Result<rot_provider::Response, ProviderError> {
            unimplemented!()
        }
    }
}
