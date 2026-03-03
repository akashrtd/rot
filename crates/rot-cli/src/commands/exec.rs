//! Single-shot exec command.

use rot_core::{
    Agent, AgentConfig, AgentRegistry, ApprovalPolicy, ContentBlock, Message, MessageId, Role,
    RuntimeSecurityConfig, SandboxMode,
};
use rot_session::{SessionEntry, SessionStore, entry_id as session_entry_id};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

/// `rot exec` output mode options.
#[derive(Debug, Clone)]
pub struct ExecOptions {
    pub json: bool,
    pub final_json: bool,
    pub output_schema: Option<String>,
}

/// Typed error used to propagate deterministic process exit codes.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ExecExitError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
struct ToolCallRecord {
    name: String,
    arguments: Value,
}

#[derive(Debug, Clone, Serialize)]
struct UsageSummary {
    input_tokens: usize,
    output_tokens: usize,
}

/// Execute a single prompt and print the result.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    prompt: &str,
    model: Option<&str>,
    provider_name: &str,
    agent_name: Option<&str>,
    resume_session_id: Option<&str>,
    fork: bool,
    rlm: bool,
    context_path: Option<&str>,
    rlm_runtime: Option<rot_rlm::RlmRuntimeKind>,
    rlm_isolation: Option<rot_rlm::RlmIsolationKind>,
    rlm_docker_image: Option<String>,
    allow_unsafe_rlm: bool,
    runtime_security: RuntimeSecurityConfig,
    options: ExecOptions,
) -> anyhow::Result<()> {
    let started = Instant::now();
    let provider = crate::provider_factory::create_provider(provider_name, model, true)?;
    let agent_profile = AgentRegistry::resolve(agent_name)?;
    let provider_label = provider.name().to_string();
    let model_label = provider.current_model().to_string();
    let sandbox_mode_label = sandbox_mode_label(runtime_security.sandbox_mode).to_string();
    let approval_policy_label = approval_policy_label(runtime_security.approval_policy).to_string();

    let (_, tools) = super::load_tool_registry(runtime_security.clone()).await?;

    let config = AgentConfig {
        agent_name: agent_profile.name.to_string(),
        system_prompt: Some(agent_profile.system_prompt.to_string()),
        max_tokens: Some(4096),
        ..Default::default()
    };

    let session_store = SessionStore::new();
    let cwd = std::env::current_dir()?;
    let (target_session_id, existing_entry_ids, mut messages) = if let Some(source_id) = resume_session_id
    {
        let source_session = session_store
            .load(&cwd, source_id)
            .await
            .map_err(|e| anyhow::anyhow!("failed to load session '{source_id}': {e}"))?;
        let source_messages = messages_from_session_entries(&source_session.entries)?;

        if fork {
            let child = session_store
                .create_child(
                    &cwd,
                    &model_label,
                    &provider_label,
                    source_id,
                    None,
                    Some(agent_profile.name),
                )
                .await
                .map_err(|e| anyhow::anyhow!("failed to fork session '{source_id}': {e}"))?;
            let existing = child
                .entries
                .iter()
                .map(|entry| session_entry_id(entry).to_string())
                .collect::<HashSet<_>>();
            (child.id, existing, source_messages)
        } else {
            let existing = source_session
                .entries
                .iter()
                .map(|entry| session_entry_id(entry).to_string())
                .collect::<HashSet<_>>();
            (source_session.id, existing, source_messages)
        }
    } else {
        if fork {
            return Err(anyhow::anyhow!("--fork requires --session <ID>"));
        }
        let session = session_store
            .create(&cwd, &model_label, &provider_label)
            .await?;
        let existing = session
            .entries
            .iter()
            .map(|entry| session_entry_id(entry).to_string())
            .collect::<HashSet<_>>();
        (session.id, existing, Vec::new())
    };

    let agent = std::sync::Arc::new(
        Agent::new(provider, tools, config, runtime_security.clone())
            .with_session_id(target_session_id.clone()),
    );

    if rlm {
        if resume_session_id.is_some() || fork {
            return Err(anyhow::anyhow!(
                "--session/--fork are not supported with --rlm in this release"
            ));
        }
        let ctx_path =
            context_path.ok_or_else(|| anyhow::anyhow!("--context is required when using --rlm"))?;
        if runtime_security.requires_explicit_rlm_opt_in() && !allow_unsafe_rlm {
            return Err(anyhow::anyhow!(
                "RLM is blocked in danger-full-access mode unless --allow-unsafe-rlm is set."
            ));
        }
        let mut config = rot_rlm::RlmConfig::default();
        config.runtime_security = runtime_security.clone();
        if let Some(runtime) = rlm_runtime {
            config.runtime = runtime;
        }
        if let Some(isolation) = rlm_isolation {
            config.isolation = isolation;
        }
        config.docker_image = rlm_docker_image;
        let runtime_label = format!("{:?}", config.runtime).to_ascii_lowercase();
        let mut engine = rot_rlm::RlmEngine::new(config, agent.clone());
        let report = engine.process_with_report(prompt, ctx_path).await?;
        let trajectory_path = report.trajectory_path.display().to_string();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let trajectory_entry = SessionEntry::Artifact {
            id: format!("artifact:rlm-trajectory:{ts}"),
            timestamp: ts,
            kind: "rlm_trajectory".to_string(),
            path: trajectory_path.clone(),
            metadata: serde_json::json!({
                "runtime": runtime_label,
                "context_path": ctx_path,
                "subcalls": report.usage.subcall_count,
            }),
        };
        session_store
            .append_by_id(&cwd, &target_session_id, trajectory_entry)
            .await
            .map_err(|e| anyhow::anyhow!("failed to append trajectory metadata: {e}"))?;
        let elapsed_ms = started.elapsed().as_millis();
        let data = ExecOutputData {
                status: "ok".to_string(),
                final_text: report.final_text,
                tool_calls: Vec::new(),
                usage: UsageSummary {
                    input_tokens: report.usage.input_tokens,
                    output_tokens: report.usage.output_tokens,
                },
                elapsed_ms,
                error: None,
                provider: provider_label,
                model: model_label,
                sandbox_mode: sandbox_mode_label,
                approval_policy: approval_policy_label,
                trajectory_path: Some(trajectory_path),
            };
        maybe_validate_schema(options.output_schema.as_deref(), &data.final_text, &options, &data)?;
        return emit_exec_output(&options, &data, &[]);
    }

    let response = match agent.process(&mut messages, prompt).await {
        Ok(resp) => resp,
        Err(err) => {
            let elapsed_ms = started.elapsed().as_millis();
            let data = ExecOutputData {
                status: "error".to_string(),
                final_text: String::new(),
                tool_calls: Vec::new(),
                usage: UsageSummary {
                    input_tokens: 0,
                    output_tokens: 0,
                },
                elapsed_ms,
                error: Some(err.to_string()),
                provider: provider_label,
                model: model_label,
                sandbox_mode: sandbox_mode_label,
                approval_policy: approval_policy_label,
                trajectory_path: None,
            };
            emit_exec_output(&options, &data, &[])?;
            return Err(anyhow::Error::new(ExecExitError {
                code: 1,
                message: "exec failed".to_string(),
            }));
        }
    };

    persist_session_delta(
        &session_store,
        &cwd,
        &target_session_id,
        &messages,
        &existing_entry_ids,
    )
    .await?;

    let elapsed_ms = started.elapsed().as_millis();
    let final_text = extract_text_from_message(&response);
    let tool_events = collect_tool_events(&messages);
    let tool_calls = tool_events
        .iter()
        .filter_map(|event| {
            if let ToolEvent::Call(call) = event {
                Some(call.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let usage = UsageSummary {
        input_tokens: 0,
        output_tokens: 0,
    };

    let data = ExecOutputData {
        status: "ok".to_string(),
        final_text,
        tool_calls,
        usage,
        elapsed_ms,
        error: None,
        provider: provider_label,
        model: model_label,
        sandbox_mode: sandbox_mode_label,
        approval_policy: approval_policy_label,
        trajectory_path: None,
    };

    maybe_validate_schema(options.output_schema.as_deref(), &data.final_text, &options, &data)?;
    emit_exec_output(&options, &data, &tool_events)?;

    Ok(())
}

#[derive(Debug, Clone)]
enum ToolEvent {
    Call(ToolCallRecord),
    Result {
        name: String,
        is_error: bool,
        output: String,
        metadata: Value,
    },
}

impl ToolEvent {
    fn to_json(&self) -> Value {
        match self {
            ToolEvent::Call(call) => serde_json::json!({
                "type": "tool_call",
                "name": call.name,
                "arguments": call.arguments,
            }),
            ToolEvent::Result {
                name,
                is_error,
                output,
                metadata,
            } => serde_json::json!({
                "type": "tool_result",
                "name": name,
                "is_error": is_error,
                "output": output,
                "metadata": metadata,
            }),
        }
    }
}

fn collect_tool_events(messages: &[Message]) -> Vec<ToolEvent> {
    let mut events = Vec::new();
    let mut calls_by_id: HashMap<String, ToolCallRecord> = HashMap::new();

    for msg in messages {
        for block in &msg.content {
            match block {
                ContentBlock::ToolCall {
                    id,
                    name,
                    arguments,
                } => {
                    let record = ToolCallRecord {
                        name: name.clone(),
                        arguments: arguments.clone(),
                    };
                    calls_by_id.insert(id.clone(), record.clone());
                    events.push(ToolEvent::Call(record));
                }
                ContentBlock::ToolResult {
                    tool_call_id,
                    content,
                    is_error,
                    metadata,
                } => {
                    let name = calls_by_id
                        .get(tool_call_id)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| "unknown".to_string());
                    events.push(ToolEvent::Result {
                        name,
                        is_error: *is_error,
                        output: content.clone(),
                        metadata: metadata.clone(),
                    });
                }
                ContentBlock::Text { .. }
                | ContentBlock::Image { .. }
                | ContentBlock::Thinking { .. } => {}
            }
        }
    }

    events
}

#[derive(Debug, Clone)]
struct ExecOutputData {
    status: String,
    final_text: String,
    tool_calls: Vec<ToolCallRecord>,
    usage: UsageSummary,
    elapsed_ms: u128,
    error: Option<String>,
    provider: String,
    model: String,
    sandbox_mode: String,
    approval_policy: String,
    trajectory_path: Option<String>,
}

fn emit_exec_output(
    options: &ExecOptions,
    data: &ExecOutputData,
    tool_events: &[ToolEvent],
) -> anyhow::Result<()> {
    if options.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "type": "session_start",
                "model": data.model,
                "provider": data.provider,
                "sandbox_mode": data.sandbox_mode,
                "approval_policy": data.approval_policy,
                "cwd": std::env::current_dir()?.display().to_string(),
            }))?
        );

        for event in tool_events {
            println!("{}", serde_json::to_string(&event.to_json())?);
        }

        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "type": "final",
                "status": data.status,
                "final_text": data.final_text,
                "usage": data.usage,
                "elapsed_ms": data.elapsed_ms,
                "error": data.error,
                "trajectory_path": data.trajectory_path,
            }))?
        );
        return Ok(());
    }

    if options.final_json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "status": data.status,
                "final_text": data.final_text,
                "tool_calls": data.tool_calls,
                "usage": data.usage,
                "elapsed_ms": data.elapsed_ms,
                "error": data.error,
                "trajectory_path": data.trajectory_path,
            }))?
        );
        return Ok(());
    }

    if !data.final_text.is_empty() {
        println!("{}", data.final_text);
    }
    Ok(())
}

fn maybe_validate_schema(
    schema_path: Option<&str>,
    final_text: &str,
    options: &ExecOptions,
    data: &ExecOutputData,
) -> anyhow::Result<()> {
    let Some(path) = schema_path else {
        return Ok(());
    };

    let validation_error = match validate_output_schema(path, final_text) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    if options.json || options.final_json {
        let mut error_data = data.clone();
        error_data.status = "error".to_string();
        error_data.error = Some(validation_error.clone());
        emit_exec_output(options, &error_data, &[])?;
    }

    Err(anyhow::Error::new(ExecExitError {
        code: 2,
        message: validation_error,
    }))
}

fn validate_output_schema(schema_path: &str, final_text: &str) -> Result<(), String> {
    let schema_raw = std::fs::read_to_string(Path::new(schema_path))
        .map_err(|e| format!("failed to read schema file '{schema_path}': {e}"))?;
    let schema: Value = serde_json::from_str(&schema_raw)
        .map_err(|e| format!("invalid schema JSON in '{schema_path}': {e}"))?;
    let output_json: Value = serde_json::from_str(final_text)
        .map_err(|e| format!("final response is not valid JSON: {e}"))?;

    let validator = jsonschema::validator_for(&schema)
        .map_err(|e| format!("invalid JSON schema: {e}"))?;

    let mut errors = validator.iter_errors(&output_json).map(|e| e.to_string()).collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        errors.sort();
        Err(format!("output schema validation failed: {}", errors.join("; ")))
    }
}

fn extract_text_from_message(msg: &Message) -> String {
    msg.content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
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

async fn persist_session_delta(
    store: &SessionStore,
    cwd: &std::path::Path,
    session_id: &str,
    messages: &[Message],
    existing_entry_ids: &HashSet<String>,
) -> anyhow::Result<()> {
    let entries = messages_to_session_entries(messages)
        .map_err(|e| anyhow::anyhow!("failed to serialize session transcript: {e}"))?;
    for entry in entries {
        let id = session_entry_id(&entry).to_string();
        if existing_entry_ids.contains(&id) {
            continue;
        }
        store
            .append_by_id(cwd, session_id, entry)
            .await
            .map_err(|e| anyhow::anyhow!("failed to append session entry: {e}"))?;
    }
    Ok(())
}

fn messages_from_session_entries(entries: &[SessionEntry]) -> anyhow::Result<Vec<Message>> {
    let mut messages = Vec::new();
    for entry in entries {
        let SessionEntry::Message {
            id,
            parent_id,
            timestamp,
            role,
            content,
        } = entry
        else {
            continue;
        };

        let role = parse_role(role)?;
        let content: Vec<ContentBlock> = serde_json::from_value(content.clone())
            .map_err(|e| anyhow::anyhow!("failed to parse message content for '{}': {e}", id))?;

        messages.push(Message {
            id: MessageId::from_string(id.clone()),
            role,
            content,
            timestamp: *timestamp,
            parent_id: parent_id
                .as_ref()
                .map(|parent| MessageId::from_string(parent.clone())),
        });
    }
    Ok(messages)
}

fn parse_role(role: &str) -> anyhow::Result<Role> {
    match role {
        "user" => Ok(Role::User),
        "assistant" => Ok(Role::Assistant),
        "tool" => Ok(Role::Tool),
        "system" => Ok(Role::System),
        other => Err(anyhow::anyhow!("unknown session message role '{}'", other)),
    }
}

fn sandbox_mode_label(mode: SandboxMode) -> &'static str {
    match mode {
        SandboxMode::ReadOnly => "read-only",
        SandboxMode::WorkspaceWrite => "workspace-write",
        SandboxMode::DangerFullAccess => "danger-full-access",
    }
}

fn approval_policy_label(policy: ApprovalPolicy) -> &'static str {
    match policy {
        ApprovalPolicy::Untrusted => "untrusted",
        ApprovalPolicy::OnRequest => "on-request",
        ApprovalPolicy::Never => "never",
    }
}

#[cfg(test)]
mod tests {
    use super::{ToolEvent, collect_tool_events, validate_output_schema};
    use rot_core::{ContentBlock, Message};

    #[test]
    fn test_collect_tool_events_order() {
        let messages = vec![
            Message::user("hello"),
            Message::assistant(vec![ContentBlock::ToolCall {
                id: "tc1".to_string(),
                name: "read".to_string(),
                arguments: serde_json::json!({"path":"README.md"}),
            }]),
            Message::tool_result_with_metadata(
                "tc1",
                "ok",
                false,
                serde_json::json!({"bytes":2}),
            ),
        ];

        let events = collect_tool_events(&messages);
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], ToolEvent::Call(_)));
        assert!(matches!(events[1], ToolEvent::Result { .. }));
    }

    #[test]
    fn test_output_schema_validation_pass_and_fail() {
        let dir = tempfile::tempdir().unwrap();
        let schema = dir.path().join("schema.json");
        std::fs::write(
            &schema,
            r#"{
                "type":"object",
                "properties":{"name":{"type":"string"}},
                "required":["name"]
            }"#,
        )
        .unwrap();

        assert!(validate_output_schema(
            schema.to_str().unwrap(),
            r#"{"name":"rot"}"#
        )
        .is_ok());
        assert!(validate_output_schema(schema.to_str().unwrap(), r#"{"age":1}"#).is_err());
    }
}
