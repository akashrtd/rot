use crate::config::RlmConfig;
use crate::context_loader::{LoadedContext, load_context};
use crate::prompts::RLM_SYSTEM_PROMPT;
use crate::python_repl::PythonReplEnv;
use crate::repl::{ReplEnv, ReplResult};
use crate::runtime::{RlmProcessPolicy, RlmRuntimeKind};
use crate::subcall::{SubcallRecord, SubcallRequest, parse_subcall_line};
use crate::trace::{ExecutionTrace, IterationTrace, RlmTrajectory, persist_trajectory, truncate_for_trace};
use crate::usage::RlmUsage;
use regex::Regex;
use rot_core::{Agent, Message};
use std::path::Path;
use std::sync::Arc;

enum RuntimeEnv {
    Bash(ReplEnv),
    Python(PythonReplEnv),
}

impl RuntimeEnv {
    async fn init(&mut self, context_path: &str) -> anyhow::Result<()> {
        match self {
            RuntimeEnv::Bash(env) => env.init(context_path).await,
            RuntimeEnv::Python(env) => env.init(context_path).await,
        }
    }

    async fn execute(&mut self, code: &str) -> anyhow::Result<ReplResult> {
        match self {
            RuntimeEnv::Bash(env) => env.execute(code).await,
            RuntimeEnv::Python(env) => env.execute(code).await,
        }
    }
}

/// Full report emitted by one RLM run.
#[derive(Debug, Clone)]
pub struct RlmRunReport {
    /// Final assistant answer.
    pub final_text: String,
    /// Aggregated usage metrics.
    pub usage: RlmUsage,
    /// Structured trajectory for this run.
    pub trajectory: RlmTrajectory,
    /// Persisted trajectory path.
    pub trajectory_path: std::path::PathBuf,
}

/// Recursive Language Model execution engine.
pub struct RlmEngine {
    config: RlmConfig,
    agent: Arc<Agent>,
    runtime: RuntimeEnv,
}

struct RunState {
    usage: RlmUsage,
    subcall_count: usize,
    subcalls: Vec<SubcallRecord>,
}

impl RunState {
    fn new() -> Self {
        Self {
            usage: RlmUsage::default(),
            subcall_count: 0,
            subcalls: Vec::new(),
        }
    }
}

impl RlmEngine {
    /// Create a new engine.
    pub fn new(config: RlmConfig, agent: Arc<Agent>) -> Self {
        let process_policy = RlmProcessPolicy::from_security(
            &config.runtime_security,
            config.isolation,
            config.docker_image.clone(),
        );
        let runtime = match config.runtime {
            RlmRuntimeKind::Python => RuntimeEnv::Python(PythonReplEnv::with_policy(process_policy)),
            RlmRuntimeKind::Bash => RuntimeEnv::Bash(ReplEnv::with_policy(process_policy)),
        };
        Self {
            config,
            agent,
            runtime,
        }
    }

    /// Process prompt and return only the final answer text.
    pub async fn process(&mut self, prompt: &str, context_path: &str) -> anyhow::Result<String> {
        let report = self.process_with_report(prompt, context_path).await?;
        Ok(report.final_text)
    }

    /// Process prompt and return full run report (usage + trajectory).
    pub async fn process_with_report(
        &mut self,
        prompt: &str,
        context_path: &str,
    ) -> anyhow::Result<RlmRunReport> {
        let run_id = ulid::Ulid::new().to_string();
        let started_at = now_secs();
        let run_start = std::time::Instant::now();

        let loaded_context = load_context(Path::new(context_path)).await?;
        self.runtime
            .init(loaded_context.extracted_path.to_string_lossy().as_ref())
            .await?;

        let mut trajectory = RlmTrajectory {
            run_id: run_id.clone(),
            started_at,
            finished_at: started_at,
            status: "running".to_string(),
            prompt: prompt.to_string(),
            context_path: loaded_context.source_path.display().to_string(),
            context_type: loaded_context.detected_type.clone(),
            runtime: format!("{:?}/{:?}", self.config.runtime, self.config.isolation)
                .to_ascii_lowercase(),
            iterations: Vec::new(),
            subcalls: Vec::new(),
            usage: RlmUsage::default(),
            final_text: None,
            error: None,
        };

        let mut state = RunState::new();
        let process_result = self
            .run_loop(prompt, &loaded_context, run_start, &mut state, &mut trajectory)
            .await;

        let finished_at = now_secs();
        trajectory.finished_at = finished_at;
        trajectory.usage = state.usage.clone();
        trajectory.subcalls = state.subcalls.clone();

        match &process_result {
            Ok(final_text) => {
                trajectory.status = "ok".to_string();
                trajectory.final_text = Some(final_text.clone());
            }
            Err(err) => {
                trajectory.status = "error".to_string();
                trajectory.error = Some(err.to_string());
            }
        }

        let trajectory_path = persist_trajectory(&trajectory, self.config.trajectory_dir.as_deref())
            .await
            .map_err(|e| anyhow::anyhow!("failed to persist RLM trajectory: {e}"))?;

        match process_result {
            Ok(final_text) => Ok(RlmRunReport {
                final_text,
                usage: state.usage,
                trajectory,
                trajectory_path,
            }),
            Err(err) => Err(anyhow::anyhow!(
                "{err} (trajectory: {})",
                trajectory_path.display()
            )),
        }
    }

    async fn run_loop(
        &mut self,
        prompt: &str,
        loaded_context: &LoadedContext,
        run_start: std::time::Instant,
        state: &mut RunState,
        trajectory: &mut RlmTrajectory,
    ) -> anyhow::Result<String> {
        let repl_block_re = Regex::new(r"```repl\n([\s\S]*?)```")?;
        let final_query_re = Regex::new(r"FINAL_ANSWER:(.*)")?;
        let final_var_re = Regex::new(r"FINAL_VAR_ANSWER:(.*)")?;

        let metadata = self.build_metadata(prompt, loaded_context);
        let mut history: Vec<Message> = vec![Message::user(format!(
            "SYSTEM INSTRUCTIONS FOR THIS TASK:\n{}\n\n{}",
            RLM_SYSTEM_PROMPT, metadata
        ))];
        let mut next_action_prompt = String::new();
        let mut current_iteration = 0;

        while current_iteration < self.config.max_iterations {
            if let Some(timeout) = self.config.max_timeout {
                if run_start.elapsed() > timeout {
                    return Err(anyhow::anyhow!("RLM engine timed out"));
                }
            }

            if let Some(cb) = &self.config.on_progress {
                cb(format!(
                    "RLM ITERATION {}/{}",
                    current_iteration + 1,
                    self.config.max_iterations
                ));
            }

            let iteration_start = std::time::Instant::now();
            let step_prompt = if current_iteration == 0 {
                metadata.clone()
            } else {
                next_action_prompt.clone()
            };

            let response_msg = self.agent.process(&mut history, &step_prompt).await?;
            let response_text = response_msg.text();
            state.usage.add_exchange(&step_prompt, &response_text);
            self.enforce_token_budget(state)?;

            let mut code_blocks = Vec::new();
            for capture in repl_block_re.captures_iter(&response_text) {
                if let Some(code) = capture.get(1) {
                    code_blocks.push(code.as_str().to_string());
                }
            }

            let (step_prompt_trace, _) = truncate_for_trace(&step_prompt, self.config.trace_max_chars);
            let mut iteration_trace = IterationTrace {
                index: current_iteration + 1,
                step_prompt: step_prompt_trace,
                code_blocks: code_blocks.clone(),
                executions: Vec::new(),
                elapsed_ms: 0,
            };

            if code_blocks.is_empty() {
                next_action_prompt = "You didn't write any ` ```repl ` code blocks. To process the context, execute runtime code or output FINAL()/FINAL_VAR(). What is your next action?".to_string();
                current_iteration += 1;
                iteration_trace.elapsed_ms = iteration_start.elapsed().as_millis();
                trajectory.iterations.push(iteration_trace);
                continue;
            }

            let mut iteration_output = String::new();
            for code in code_blocks {
                let mut repl_result = self.runtime.execute(&code).await?;
                let (resolved_stdout, subcall_ids) = self
                    .resolve_subcalls(&repl_result.stdout, 1, state)
                    .await?;
                repl_result.stdout = resolved_stdout;

                iteration_output.push_str(&format!(
                    "$ {}\n> stdout:\n{}\n> stderr:\n{}\n> Exit Code: {:?}\n\n",
                    code.trim(),
                    repl_result.stdout.trim(),
                    repl_result.stderr.trim(),
                    repl_result.exit_code
                ));

                let (stdout_trace, stdout_truncated) =
                    truncate_for_trace(&repl_result.stdout, self.config.trace_max_chars);
                let (stderr_trace, stderr_truncated) =
                    truncate_for_trace(&repl_result.stderr, self.config.trace_max_chars);
                iteration_trace.executions.push(ExecutionTrace {
                    code: code.clone(),
                    stdout: stdout_trace,
                    stderr: stderr_trace,
                    exit_code: repl_result.exit_code,
                    truncated: stdout_truncated || stderr_truncated,
                    subcall_ids,
                });

                if let Some(capture) = final_query_re.captures(&repl_result.stdout) {
                    if let Some(answer) = capture.get(1) {
                        iteration_trace.elapsed_ms = iteration_start.elapsed().as_millis();
                        trajectory.iterations.push(iteration_trace);
                        return Ok(answer.as_str().to_string());
                    }
                }

                if let Some(capture) = final_var_re.captures(&repl_result.stdout) {
                    if let Some(value) = capture.get(1) {
                        iteration_trace.elapsed_ms = iteration_start.elapsed().as_millis();
                        trajectory.iterations.push(iteration_trace);
                        return Ok(value.as_str().to_string());
                    }
                }
            }

            let mut final_out = iteration_output;
            if final_out.len() > 10_000 {
                let trunc_msg = "\n...[output truncated due to length]...\n";
                let start_part = &final_out[..5_000];
                let end_part = &final_out[final_out.len() - 5_000..];
                final_out = format!("{}{}{}", start_part, trunc_msg, end_part);
            }

            next_action_prompt = format!(
                "Execution Results:\n```\n{}\n```\nWhat is your next action? (Analyze results or call FINAL/FINAL_VAR)",
                final_out
            );
            current_iteration += 1;

            iteration_trace.elapsed_ms = iteration_start.elapsed().as_millis();
            trajectory.iterations.push(iteration_trace);
        }

        Err(anyhow::anyhow!(
            "RLM max iterations reached without FINAL/FINAL_VAR"
        ))
    }

    async fn resolve_subcalls(
        &self,
        text: &str,
        initial_depth: usize,
        state: &mut RunState,
    ) -> anyhow::Result<(String, Vec<String>)> {
        let mut depth = initial_depth;
        let mut current = text.to_string();
        let mut all_ids = Vec::new();

        loop {
            let mut found = false;
            let mut next_lines = Vec::new();

            for line in current.lines() {
                match parse_subcall_line(line)? {
                    Some(req) => {
                        found = true;
                        let (id, replacement) = self.execute_subcall(req, depth, state).await?;
                        all_ids.push(id);
                        next_lines.push(replacement);
                    }
                    None => next_lines.push(line.to_string()),
                }
            }

            if !found {
                return Ok((current, all_ids));
            }

            current = next_lines.join("\n");
            depth += 1;

            if depth > self.config.max_subcall_depth && current.lines().any(is_subcall_line) {
                return Err(anyhow::anyhow!(
                    "RLM subcall recursion depth exceeded (max {})",
                    self.config.max_subcall_depth
                ));
            }
        }
    }

    async fn execute_subcall(
        &self,
        request: SubcallRequest,
        depth: usize,
        state: &mut RunState,
    ) -> anyhow::Result<(String, String)> {
        if depth > self.config.max_subcall_depth {
            return Err(anyhow::anyhow!(
                "RLM subcall recursion depth exceeded (max {})",
                self.config.max_subcall_depth
            ));
        }

        if state.subcall_count >= self.config.max_subcalls {
            return Err(anyhow::anyhow!(
                "RLM subcall budget exceeded: max_subcalls={} reached",
                self.config.max_subcalls
            ));
        }

        let mut prompt = request.query.clone();
        if let Some(input) = &request.input {
            prompt.push_str("\n\nSubcall input:\n");
            prompt.push_str(input);
        }

        let subcall_id = ulid::Ulid::new().to_string();
        let started = std::time::Instant::now();
        state.subcall_count += 1;
        state.usage.subcall_count += 1;

        let sub_agent = self.config.subcall_agent.as_ref().unwrap_or(&self.agent);
        let mut messages = Vec::new();
        let timed = tokio::time::timeout(self.config.subcall_timeout, async {
            let msg = sub_agent.process(&mut messages, &prompt).await?;
            Ok::<_, anyhow::Error>(msg.text())
        })
        .await;

        let (response, error) = match timed {
            Ok(Ok(text)) => (text, None),
            Ok(Err(err)) => (String::new(), Some(err.to_string())),
            Err(_) => (
                String::new(),
                Some(format!(
                    "subcall timed out after {}s",
                    self.config.subcall_timeout.as_secs()
                )),
            ),
        };

        state.usage.add_exchange(&prompt, &response);
        self.enforce_token_budget(state)?;

        let record = SubcallRecord {
            id: subcall_id.clone(),
            depth,
            request: request.clone(),
            response: response.clone(),
            error: error.clone(),
            elapsed_ms: started.elapsed().as_millis(),
            input_tokens: crate::usage::estimate_tokens(&prompt),
            output_tokens: crate::usage::estimate_tokens(&response),
        };
        state.subcalls.push(record);

        if let Some(err) = error {
            return Err(anyhow::anyhow!("RLM subcall failed: {err}"));
        }

        Ok((subcall_id, response))
    }

    fn enforce_token_budget(&self, state: &RunState) -> anyhow::Result<()> {
        if let Some(limit) = self.config.max_total_tokens {
            if state.usage.total_tokens() > limit {
                return Err(anyhow::anyhow!(
                    "RLM token budget exceeded: {} > {}",
                    state.usage.total_tokens(),
                    limit
                ));
            }
        }
        Ok(())
    }

    fn build_metadata(&self, prompt: &str, context: &LoadedContext) -> String {
        format!(
            r#"TASK:
{}

Context metadata:
- source_path: {}
- detected_type: {}
- extracted_length: {}
- runtime: {:?}
- isolation: {:?}
- sandbox_mode: {:?}
- network_access: {}

The preprocessed context is available in runtime helpers.
Begin by running context_preview() and context_length() before deeper analysis."#,
            prompt,
            context.source_path.display(),
            context.detected_type,
            context.extracted_length(),
            self.config.runtime,
            self.config.isolation,
            self.config.runtime_security.sandbox_mode,
            self.config.runtime_security.sandbox_network_access
        )
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn is_subcall_line(line: &str) -> bool {
    line.starts_with(crate::subcall::SUBLM_MARKER)
}

#[cfg(test)]
mod tests {
    use super::{RlmEngine, is_subcall_line};
    use crate::RlmConfig;
    use async_trait::async_trait;
    use futures::stream::{self, BoxStream, StreamExt};
    use rot_core::{Agent, AgentConfig, RuntimeSecurityConfig};
    use rot_provider::{
        ModelInfo, Provider, ProviderError, Request, Response, StopReason, StreamEvent, Usage,
    };
    use rot_tools::ToolRegistry;
    use std::sync::{Arc, Mutex};

    struct ScriptedProvider {
        responses: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl Provider for ScriptedProvider {
        fn name(&self) -> &str {
            "mock"
        }

        fn models(&self) -> Vec<ModelInfo> {
            vec![ModelInfo {
                id: "mock-model".to_string(),
                name: "Mock Model".to_string(),
                context_window: 8192,
                max_output_tokens: 4096,
                supports_thinking: false,
                supports_tools: true,
            }]
        }

        fn current_model(&self) -> &str {
            "mock-model"
        }

        fn set_model(&mut self, _model: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn stream(
            &self,
            _request: Request,
        ) -> Result<BoxStream<'_, Result<StreamEvent, ProviderError>>, ProviderError> {
            let text = self
                .responses
                .lock()
                .unwrap()
                .remove(0);
            Ok(stream::iter(vec![
                Ok(StreamEvent::TextDelta { delta: text }),
                Ok(StreamEvent::Done {
                    reason: StopReason::EndTurn,
                }),
            ])
            .boxed())
        }

        async fn complete(&self, _request: Request) -> Result<Response, ProviderError> {
            Ok(Response {
                content: Vec::new(),
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
            })
        }
    }

    #[test]
    fn test_is_subcall_line() {
        assert!(is_subcall_line("__ROT_SUBLM__{}"));
        assert!(!is_subcall_line("hello"));
    }

    #[tokio::test]
    async fn test_subcall_budget_exceeded() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }

        let responses = Arc::new(Mutex::new(vec![
            "```repl\nSUBLM('q1', 'a')\nSUBLM('q2', 'b')\nFINAL('done')\n```".to_string(),
            "sub1".to_string(),
            "sub2".to_string(),
        ]));

        let provider = Box::new(ScriptedProvider { responses });
        let agent = Arc::new(Agent::new(
            provider,
            ToolRegistry::new(),
            AgentConfig::default(),
            RuntimeSecurityConfig::default(),
        ));

        let cfg = RlmConfig {
            max_subcalls: 1,
            ..Default::default()
        };

        let mut engine = RlmEngine::new(cfg, agent);
        let dir = tempfile::tempdir().unwrap();
        let ctx = dir.path().join("ctx.txt");
        std::fs::write(&ctx, "ctx").unwrap();

        let err = engine
            .process_with_report("test", ctx.to_string_lossy().as_ref())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("max_subcalls"));
    }
}
