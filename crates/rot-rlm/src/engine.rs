use crate::repl::ReplEnv;
use crate::prompts::RLM_SYSTEM_PROMPT;
use rot_core::{Agent, Message};
use std::sync::Arc;
use regex::Regex;

/// RLM configuration
pub struct RlmConfig {
    pub max_iterations: usize,
    pub max_timeout: Option<std::time::Duration>,
    pub on_progress: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

impl Default for RlmConfig {
    fn default() -> Self {
        Self {
            max_iterations: 30,
            max_timeout: Some(std::time::Duration::from_secs(300)),
            on_progress: None,
        }
    }
}

pub struct RlmEngine {
    config: RlmConfig,
    agent: Arc<Agent>,
    repl: ReplEnv,
}

impl RlmEngine {
    pub fn new(config: RlmConfig, agent: Arc<Agent>) -> Self {
        Self {
            config,
            agent,
            repl: ReplEnv::new(),
        }
    }

    /// Extracted helper to process sub-llm queries from text output
    async fn process_llm_queries(&self, text: &str) -> anyhow::Result<String> {
        let mut result = text.to_string();
        
        let query_regex = Regex::new(r"LLM_QUERY:(.*):END_QUERY").unwrap();
        
        for capture in query_regex.captures_iter(text) {
            if let Some(query) = capture.get(1) {
                let prompt = query.as_str();
                
                // Do a quick, headless sub-provider query
                let mut messages = Vec::new();
                if let Ok(response) = self.agent.process(&mut messages, prompt).await {
                    let text_response = response.text();
                    result = result.replace(&capture[0], &text_response);
                }
            }
        }
        
        Ok(result)
    }

    /// Process a prompt using RLM with dynamic context
    pub async fn process(&mut self, prompt: &str, context_path: &str) -> anyhow::Result<String> {
        let start = std::time::Instant::now();

        // 1. Initialize REPL with context
        self.repl.init(context_path).await?;
        
        let repl_block_re = Regex::new(r"```repl\n([\s\S]*?)```").unwrap();
        let final_query_re = Regex::new(r"FINAL_ANSWER:(.*)").unwrap();

        let metadata = self.build_metadata(prompt);
        let mut history: Vec<Message> = vec![
            Message::user(format!("SYSTEM INSTRUCTIONS FOR THIS TASK:\n{}\n\n{}", RLM_SYSTEM_PROMPT, metadata))
        ];
        let mut next_action_prompt = String::new();
        let mut current_iteration = 0;

        while current_iteration < self.config.max_iterations {
            if let Some(timeout) = self.config.max_timeout {
                if start.elapsed() > timeout {
                    return Err(anyhow::anyhow!("RLM Engine Timed out"));
                }
            }
            
            if let Some(cb) = &self.config.on_progress {
                cb(format!("RLM ITERATION {}/{}", current_iteration + 1, self.config.max_iterations));
            }

            // To properly let Agent drive, we just call process
            let step_prompt = if current_iteration == 0 {
                metadata.clone()
            } else {
                next_action_prompt.clone()
            };

            let response_msg = self.agent.process(&mut history, &step_prompt).await?;
            let response_text = response_msg.text();

            // Extract code blocks
            let mut code_blocks = Vec::new();
            for capture in repl_block_re.captures_iter(&response_text) {
                if let Some(code) = capture.get(1) {
                    code_blocks.push(code.as_str().to_string());
                }
            }

            if code_blocks.is_empty() {
                // If it didn't write code, ask it to write code or conclude
                next_action_prompt = "You didn't write any ` ```repl ` code blocks. To process the context, you must execute bash commands, or output a FINAL response. What is your next action?".to_string();
                current_iteration += 1;
                continue;
            }

            // Execute code blocks sequentially
            let mut iteration_output = String::new();
            for code in code_blocks {
                let mut repl_result = self.repl.execute(&code).await?;
                
                // Process potential `llm_query` responses returned via stdout
                repl_result.stdout = self.process_llm_queries(&repl_result.stdout).await?;
                
                iteration_output.push_str(&format!("$ {}\n> stdout:\n{}\n> stderr:\n{}\n> Exit Code: {:?}\n\n", 
                    code.trim(), 
                    repl_result.stdout.trim(), 
                    repl_result.stderr.trim(), 
                    repl_result.exit_code
                ));

                // Check for FINAL Answer
                if let Some(capture) = final_query_re.captures(&repl_result.stdout) {
                    if let Some(answer) = capture.get(1) {
                        return Ok(answer.as_str().to_string());
                    }
                }
            }

            // Limit response size if stdout was massive
            let mut final_out = iteration_output;
            if final_out.len() > 10000 {
                let trunc_msg = "\n...[output truncated due to length]...\n";
                let start_part = &final_out[..5000];
                let end_part = &final_out[final_out.len() - 5000..];
                final_out = format!("{}{}{}", start_part, trunc_msg, end_part);
            }

            next_action_prompt = format!(
                "Execution Results:\n```\n{}\n```\nWhat is your next action? (Analyze the results or call FINAL())",
                final_out
            );
            
            current_iteration += 1;
        }

        Err(anyhow::anyhow!("RLM Max Iterations Reached without calling FINAL"))
    }

    fn build_metadata(&self, prompt: &str) -> String {
        format!(
            r#"TASK:
{}

The target context is readily available via the $CONTEXT_FILE bash variable.
Begin by executing `context_preview()` and `context_length()` inside a repl block to understand the data before answering the task."#,
            prompt
        )
    }
}
