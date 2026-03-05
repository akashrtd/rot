//! Headless HTTP service mode for `rot`.

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use rot_core::{
    Agent, AgentConfig, AgentRegistry, ApprovalPolicy, ContentBlock, Message, RuntimeSecurityConfig,
    SandboxMode,
};
use rot_provider::{
    AnthropicProvider, Provider, new_google_provider, new_ollama_provider, new_openai_provider,
    new_openrouter_provider, new_zai_provider,
};
use rot_session::SessionStore;
use rot_tools::ToolRegistry;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;

/// Service startup configuration.
#[derive(Debug, Clone)]
pub struct ServeOptions {
    /// Bind address, e.g. `127.0.0.1:7878`.
    pub bind: String,
    /// Default provider.
    pub provider: String,
    /// Optional default model.
    pub model: Option<String>,
    /// Optional default agent.
    pub agent: Option<String>,
    /// Runtime security policy.
    pub runtime_security: RuntimeSecurityConfig,
}

#[derive(Debug, Clone)]
struct AppState {
    defaults: ServeOptions,
}

#[derive(Debug, Deserialize)]
struct ExecRequest {
    prompt: String,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    agent: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct ToolCallRecord {
    name: String,
    arguments: Value,
}

#[derive(Debug, Serialize)]
struct UsageSummary {
    input_tokens: usize,
    output_tokens: usize,
}

#[derive(Debug, Serialize)]
struct ExecResponse {
    status: String,
    final_text: String,
    tool_calls: Vec<ToolCallRecord>,
    usage: UsageSummary,
    elapsed_ms: u128,
    error: Option<String>,
}

/// Run the HTTP service until interrupted.
pub async fn run(options: ServeOptions) -> anyhow::Result<()> {
    let state = Arc::new(AppState { defaults: options.clone() });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/exec", post(exec_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&options.bind).await?;
    println!("rot serve listening on http://{}", options.bind);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_handler() -> Json<Value> {
    Json(serde_json::json!({"status":"ok"}))
}

async fn exec_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExecRequest>,
) -> Json<ExecResponse> {
    match run_exec(&state.defaults, req).await {
        Ok(resp) => Json(resp),
        Err(err) => Json(ExecResponse {
            status: "error".to_string(),
            final_text: String::new(),
            tool_calls: Vec::new(),
            usage: UsageSummary {
                input_tokens: 0,
                output_tokens: 0,
            },
            elapsed_ms: 0,
            error: Some(err.to_string()),
        }),
    }
}

async fn run_exec(defaults: &ServeOptions, req: ExecRequest) -> anyhow::Result<ExecResponse> {
    if defaults.runtime_security.approval_policy != ApprovalPolicy::Never {
        return Err(anyhow::anyhow!(
            "serve mode requires approval policy 'never' (non-interactive)"
        ));
    }

    let started = Instant::now();
    let provider_name = req.provider.unwrap_or_else(|| defaults.provider.clone());
    let model_name = req.model.as_deref().or(defaults.model.as_deref());
    let agent_name = req.agent.as_deref().or(defaults.agent.as_deref());

    let provider = create_provider(&provider_name, model_name)?;
    let (_, tools) = load_tool_registry(defaults.runtime_security.clone()).await?;

    let agent_profile = AgentRegistry::resolve(agent_name)?;
    let config = AgentConfig {
        agent_name: agent_profile.name.to_string(),
        system_prompt: Some(agent_profile.system_prompt.to_string()),
        max_tokens: Some(4096),
        ..Default::default()
    };

    let cwd = std::env::current_dir()?;
    let session_store = SessionStore::new();
    let session = session_store
        .create(&cwd, provider.current_model(), provider.name())
        .await?;
    let agent = Arc::new(
        Agent::new(provider, tools, config, defaults.runtime_security.clone())
            .with_session_id(session.id),
    );

    let mut messages = Vec::new();
    let response = agent.process(&mut messages, &req.prompt).await?;
    let elapsed_ms = started.elapsed().as_millis();

    let final_text = response
        .content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::Text { text } = block {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("");

    let tool_calls = collect_tool_calls(&messages);

    Ok(ExecResponse {
        status: "ok".to_string(),
        final_text,
        tool_calls,
        usage: UsageSummary {
            input_tokens: 0,
            output_tokens: 0,
        },
        elapsed_ms,
        error: None,
    })
}

fn collect_tool_calls(messages: &[Message]) -> Vec<ToolCallRecord> {
    let mut out = Vec::new();
    for msg in messages {
        for block in &msg.content {
            if let ContentBlock::ToolCall {
                name, arguments, ..
            } = block
            {
                out.push(ToolCallRecord {
                    name: name.clone(),
                    arguments: arguments.clone(),
                });
            }
        }
    }
    out
}

async fn load_tool_registry(
    runtime_security: RuntimeSecurityConfig,
) -> anyhow::Result<(rot_core::Config, ToolRegistry)> {
    let config_store = rot_core::ConfigStore::new();
    let config = config_store.load();

    let mut tools = ToolRegistry::new();
    rot_tools::register_all(&mut tools);
    rot_tools::register_custom_tools(&mut tools, &config.custom_tools)
        .map_err(|e| anyhow::anyhow!("Failed to load custom tools: {e}"))?;
    rot_tools::register_mcp_tools(
        &mut tools,
        &config.mcp_servers,
        &std::env::current_dir()?,
        match runtime_security.sandbox_mode {
            SandboxMode::ReadOnly => rot_tools::SandboxMode::ReadOnly,
            SandboxMode::WorkspaceWrite => rot_tools::SandboxMode::WorkspaceWrite,
            SandboxMode::DangerFullAccess => rot_tools::SandboxMode::DangerFullAccess,
        },
        runtime_security.sandbox_network_access
            || runtime_security.sandbox_mode == SandboxMode::DangerFullAccess,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to load MCP tools: {e}"))?;

    Ok((config, tools))
}

fn create_provider(provider_name: &str, model: Option<&str>) -> anyhow::Result<Box<dyn Provider>> {
    let mut provider: Box<dyn Provider> = match provider_name {
        "anthropic" => Box::new(AnthropicProvider::new(required_env("ANTHROPIC_API_KEY")?, vec![])),
        "zai" => Box::new(new_zai_provider(required_env("ZAI_API_KEY")?, vec![])),
        "openai" => Box::new(new_openai_provider(required_env("OPENAI_API_KEY")?, vec![])),
        "ollama" => Box::new(new_ollama_provider(String::new(), vec![])),
        "openrouter" => Box::new(new_openrouter_provider(required_env("OPENROUTER_API_KEY")?, vec![])),
        "google" => {
            let key = std::env::var("GOOGLE_API_KEY")
                .or_else(|_| std::env::var("GEMINI_API_KEY"))
                .map_err(|_| anyhow::anyhow!("GOOGLE_API_KEY or GEMINI_API_KEY not set"))?;
            Box::new(new_google_provider(key, vec![]))
        }
        other => {
            return Err(anyhow::anyhow!(
                "Unknown provider: {other}. Available: anthropic, zai, openai, ollama, openrouter, google"
            ))
        }
    };

    if let Some(model) = model {
        provider.set_model(model)?;
    }
    Ok(provider)
}

fn required_env(name: &str) -> anyhow::Result<String> {
    std::env::var(name).map_err(|_| anyhow::anyhow!("{name} not set"))
}

#[cfg(test)]
mod tests {
    use super::collect_tool_calls;
    use rot_core::{ContentBlock, Message};

    #[test]
    fn test_collect_tool_calls() {
        let messages = vec![Message::assistant(vec![ContentBlock::ToolCall {
            id: "tc1".to_string(),
            name: "read".to_string(),
            arguments: serde_json::json!({"path":"README.md"}),
        }])];
        let calls = collect_tool_calls(&messages);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");
    }
}
