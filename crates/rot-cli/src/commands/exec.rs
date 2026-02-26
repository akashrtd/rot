//! Single-shot exec command.

use rot_core::{Agent, AgentConfig, ApprovalPolicy, ContentBlock, Message, RuntimeSecurityConfig, SandboxMode};
use rot_provider::{AnthropicProvider, Provider, new_openai_provider, new_zai_provider};
use rot_tools::ToolRegistry;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
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
pub async fn run(
    prompt: &str,
    model: Option<&str>,
    provider_name: &str,
    rlm: bool,
    context_path: Option<&str>,
    runtime_security: RuntimeSecurityConfig,
    options: ExecOptions,
) -> anyhow::Result<()> {
    let started = Instant::now();
    let provider = create_provider(provider_name, model)?;
    let provider_label = provider.name().to_string();
    let model_label = provider.current_model().to_string();
    let sandbox_mode_label = sandbox_mode_label(runtime_security.sandbox_mode).to_string();
    let approval_policy_label = approval_policy_label(runtime_security.approval_policy).to_string();

    let mut tools = ToolRegistry::new();
    rot_tools::register_all(&mut tools);

    let config = AgentConfig {
        system_prompt: Some(
            "You are rot, an AI coding assistant. Be concise and direct.".to_string(),
        ),
        max_tokens: Some(4096),
        ..Default::default()
    };

    let agent = std::sync::Arc::new(Agent::new(provider, tools, config, runtime_security.clone()));

    if rlm {
        let ctx_path =
            context_path.ok_or_else(|| anyhow::anyhow!("--context is required when using --rlm"))?;
        let config = rot_rlm::RlmConfig::default();
        let mut engine = rot_rlm::RlmEngine::new(config, agent.clone());
        let final_text = engine.process(prompt, ctx_path).await?;
        let elapsed_ms = started.elapsed().as_millis();
        let data = ExecOutputData {
                status: "ok".to_string(),
                final_text,
                tool_calls: Vec::new(),
                usage: UsageSummary {
                    input_tokens: 0,
                    output_tokens: 0,
                },
                elapsed_ms,
                error: None,
                provider: provider_label,
                model: model_label,
                sandbox_mode: sandbox_mode_label,
                approval_policy: approval_policy_label,
            };
        maybe_validate_schema(options.output_schema.as_deref(), &data.final_text, &options, &data)?;
        return emit_exec_output(&options, &data, &[]);
    }

    let mut messages: Vec<Message> = Vec::new();
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
            };
            emit_exec_output(&options, &data, &[])?;
            return Err(anyhow::Error::new(ExecExitError {
                code: 1,
                message: "exec failed".to_string(),
            }));
        }
    };

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

fn create_provider(provider_name: &str, model: Option<&str>) -> anyhow::Result<Box<dyn Provider>> {
    match provider_name {
        "anthropic" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                anyhow::anyhow!(
                    "ANTHROPIC_API_KEY not set. Set it with:\n  \
                     export ANTHROPIC_API_KEY=your-key-here"
                )
            })?;
            let mut provider = AnthropicProvider::new(api_key);
            if let Some(m) = model {
                provider.set_model(m).map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            Ok(Box::new(provider))
        }
        "zai" => {
            let api_key = std::env::var("ZAI_API_KEY").map_err(|_| {
                anyhow::anyhow!(
                    "ZAI_API_KEY not set. Set it with:\n  \
                     export ZAI_API_KEY=your-key-here\n\n\
                     Get your key from https://z.ai"
                )
            })?;
            let mut provider = new_zai_provider(api_key);
            if let Some(m) = model {
                provider.set_model(m).map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            Ok(Box::new(provider))
        }
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
                anyhow::anyhow!(
                    "OPENAI_API_KEY not set. Set it with:\n  \
                     export OPENAI_API_KEY=your-key-here"
                )
            })?;
            let mut provider = new_openai_provider(api_key);
            if let Some(m) = model {
                provider.set_model(m).map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            Ok(Box::new(provider))
        }
        other => Err(anyhow::anyhow!(
            "Unknown provider: {other}. Available: anthropic, zai, openai"
        )),
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
