use crate::{RlmConfig, RlmEngine};
use async_trait::async_trait;
use futures::stream::{self, BoxStream, StreamExt};
use rot_core::{Agent, AgentConfig, RuntimeSecurityConfig};
use rot_provider::{
    ModelInfo, Provider, ProviderContent, ProviderError, Request, Response, StopReason,
    StreamEvent, Usage,
};
use rot_tools::ToolRegistry;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct ScriptedProvider {
    responses: Arc<Mutex<VecDeque<String>>>,
    call_count: Arc<AtomicUsize>,
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
        self.call_count.fetch_add(1, Ordering::Relaxed);
        let text = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_default();
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
            content: vec![ProviderContent::Text {
                text: String::new(),
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
        })
    }
}

fn python_available() -> bool {
    std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_ok()
}

fn build_agent(script: &[&str]) -> (Arc<Agent>, Arc<AtomicUsize>) {
    let provider = Box::new(ScriptedProvider {
        responses: Arc::new(Mutex::new(
            script.iter().map(|s| s.to_string()).collect::<VecDeque<_>>(),
        )),
        call_count: Arc::new(AtomicUsize::new(0)),
    });

    let calls = provider.call_count.clone();
    let agent = Arc::new(Agent::new(
        provider,
        ToolRegistry::new(),
        AgentConfig::default(),
        RuntimeSecurityConfig::default(),
    ));

    (agent, calls)
}

fn write_context() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ctx.txt");
    std::fs::write(&path, "alpha beta gamma delta epsilon zeta eta theta").unwrap();
    (dir, path.to_string_lossy().to_string())
}

#[tokio::test]
async fn test_root_step_executes_and_finalizes() {
    if !python_available() {
        return;
    }

    let (dir, ctx) = write_context();
    let (agent, calls) = build_agent(&["```repl\nprint(context_preview())\nFINAL('done')\n```"]);

    let mut engine = RlmEngine::new(RlmConfig::default(), agent);
    let report = engine.process_with_report("analyze", &ctx).await.unwrap();
    drop(dir);

    assert_eq!(report.final_text, "done");
    assert_eq!(report.trajectory.iterations.len(), 1);
    assert_eq!(report.trajectory.iterations[0].executions.len(), 1);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn test_no_code_retry_path() {
    if !python_available() {
        return;
    }

    let (_dir, ctx) = write_context();
    let (agent, calls) = build_agent(&[
        "I will inspect first and then run code.",
        "```repl\nFINAL('after retry')\n```",
    ]);

    let cfg = RlmConfig {
        max_iterations: 4,
        ..Default::default()
    };

    let mut engine = RlmEngine::new(cfg, agent);
    let report = engine.process_with_report("analyze", &ctx).await.unwrap();

    assert_eq!(report.final_text, "after retry");
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_eq!(report.trajectory.iterations.len(), 2);
    assert!(report.trajectory.iterations[0].code_blocks.is_empty());
}

#[tokio::test]
async fn test_final_var_success() {
    if !python_available() {
        return;
    }

    let (_dir, ctx) = write_context();
    let (agent, _calls) = build_agent(&[
        "```repl\nresult = {'answer': 42}\nFINAL_VAR('result')\n```",
    ]);

    let mut engine = RlmEngine::new(RlmConfig::default(), agent);
    let report = engine.process_with_report("extract", &ctx).await.unwrap();

    assert!(report.final_text.contains("answer"));
    assert!(report.final_text.contains("42"));
}

#[tokio::test]
async fn test_recursive_subcall_success() {
    if !python_available() {
        return;
    }

    let (_dir, ctx) = write_context();
    let (agent, calls) = build_agent(&[
        "```repl\nSUBLM('Summarize this', context_slice(0, 12))\nFINAL('ok')\n```",
        "sub-summary",
    ]);

    let mut engine = RlmEngine::new(RlmConfig::default(), agent);
    let report = engine.process_with_report("analyze", &ctx).await.unwrap();

    assert_eq!(report.final_text, "ok");
    assert_eq!(report.usage.subcall_count, 1);
    assert_eq!(report.trajectory.subcalls.len(), 1);
    assert_eq!(report.trajectory.subcalls[0].response, "sub-summary");
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn test_recursion_depth_exceeded() {
    if !python_available() {
        return;
    }

    let (_dir, ctx) = write_context();
    let (agent, _calls) = build_agent(&[
        "```repl\nSUBLM('outer', 'x')\n```",
        "__ROT_SUBLM__{\"query\":\"inner\",\"input\":\"y\"}",
    ]);

    let cfg = RlmConfig {
        max_subcall_depth: 1,
        ..Default::default()
    };

    let mut engine = RlmEngine::new(cfg, agent);
    let err = engine.process_with_report("deep", &ctx).await.unwrap_err();
    assert!(err.to_string().contains("recursion depth exceeded"));
}

#[tokio::test]
async fn test_timeout_exceeded() {
    if !python_available() {
        return;
    }

    let (_dir, ctx) = write_context();
    let (agent, _calls) = build_agent(&[
        "```repl\nimport time\ntime.sleep(0.05)\n```",
    ]);

    let cfg = RlmConfig {
        max_timeout: Some(std::time::Duration::from_millis(10)),
        ..Default::default()
    };

    let mut engine = RlmEngine::new(cfg, agent);
    let err = engine.process_with_report("slow", &ctx).await.unwrap_err();
    assert!(err.to_string().contains("timed out"));
}

#[tokio::test]
async fn test_truncation_behavior() {
    if !python_available() {
        return;
    }

    let (_dir, ctx) = write_context();
    let (agent, _calls) = build_agent(&[
        "```repl\nprint('x' * 12050)\n```",
        "```repl\nFINAL('ok')\n```",
    ]);

    let cfg = RlmConfig {
        trace_max_chars: 7000,
        ..Default::default()
    };

    let mut engine = RlmEngine::new(cfg, agent);
    let report = engine.process_with_report("truncate", &ctx).await.unwrap();

    assert_eq!(report.final_text, "ok");
    assert!(report.trajectory.iterations.len() >= 2);

    let first_exec = &report.trajectory.iterations[0].executions[0];
    assert!(first_exec.truncated);
    assert!(first_exec.stdout.contains("...[truncated]..."));

    let second_prompt = &report.trajectory.iterations[1].step_prompt;
    assert!(second_prompt.contains("...[output truncated due to length]..."));
}

#[tokio::test]
async fn test_subcall_count_budget_exceeded() {
    if !python_available() {
        return;
    }

    let (_dir, ctx) = write_context();
    let (agent, _calls) = build_agent(&[
        "```repl\nSUBLM('q1', 'a')\nSUBLM('q2', 'b')\nFINAL('done')\n```",
        "sub1",
    ]);

    let cfg = RlmConfig {
        max_subcalls: 1,
        ..Default::default()
    };

    let mut engine = RlmEngine::new(cfg, agent);
    let err = engine.process_with_report("budget", &ctx).await.unwrap_err();
    assert!(err.to_string().contains("max_subcalls"));
}
