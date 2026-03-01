//! CLI argument and command definitions.

use clap::{Parser, Subcommand, ValueEnum};
use rot_core::config::Config;
use rot_core::security::{ApprovalPolicy, RuntimeSecurityConfig, SandboxMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ApprovalPolicyArg {
    Untrusted,
    OnRequest,
    Never,
}

impl From<ApprovalPolicyArg> for ApprovalPolicy {
    fn from(value: ApprovalPolicyArg) -> Self {
        match value {
            ApprovalPolicyArg::Untrusted => ApprovalPolicy::Untrusted,
            ApprovalPolicyArg::OnRequest => ApprovalPolicy::OnRequest,
            ApprovalPolicyArg::Never => ApprovalPolicy::Never,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SandboxModeArg {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl From<SandboxModeArg> for SandboxMode {
    fn from(value: SandboxModeArg) -> Self {
        match value {
            SandboxModeArg::ReadOnly => SandboxMode::ReadOnly,
            SandboxModeArg::WorkspaceWrite => SandboxMode::WorkspaceWrite,
            SandboxModeArg::DangerFullAccess => SandboxMode::DangerFullAccess,
        }
    }
}

#[derive(Parser)]
#[command(name = "rot", version, about = "Recursive Operations Tool — AI coding agent")]
pub struct Cli {
    /// LLM provider to use.
    #[arg(long, default_value = "anthropic", global = true)]
    pub provider: String,

    /// Built-in agent profile to use.
    #[arg(long, global = true)]
    pub agent: Option<String>,

    /// Model to use (defaults to provider's default model).
    #[arg(long, global = true)]
    pub model: Option<String>,

    /// Enable verbose logging.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Sandbox mode for tool execution.
    #[arg(long, global = true, value_enum)]
    pub sandbox: Option<SandboxModeArg>,

    /// Approval policy for tool execution.
    #[arg(long = "ask-for-approval", global = true, value_enum)]
    pub ask_for_approval: Option<ApprovalPolicyArg>,

    /// Shortcut for --sandbox workspace-write --ask-for-approval on-request.
    #[arg(
        long,
        global = true,
        conflicts_with_all = ["sandbox", "ask_for_approval", "dangerously_bypass_approvals_and_sandbox"]
    )]
    pub full_auto: bool,

    /// Disable sandbox and approvals (DANGEROUS). Alias: --yolo.
    #[arg(
        long,
        visible_alias = "yolo",
        global = true,
        conflicts_with_all = ["full_auto", "sandbox", "ask_for_approval"]
    )]
    pub dangerously_bypass_approvals_and_sandbox: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start an interactive chat session (default).
    Chat,

    /// Execute a single prompt and exit.
    Exec {
        /// The prompt to execute.
        prompt: String,

        /// Run using the Recursive Language Model (RLM) engine for huge contexts.
        #[arg(long)]
        rlm: bool,

        /// External context file to map into the RLM environment (required if --rlm is used)
        #[arg(long, requires = "rlm")]
        context: Option<String>,

        /// Emit JSONL events to stdout.
        #[arg(long, conflicts_with = "final_json")]
        json: bool,

        /// Emit one final JSON object to stdout.
        #[arg(long, conflicts_with = "json")]
        final_json: bool,

        /// JSON Schema file used to validate final output JSON shape.
        #[arg(long)]
        output_schema: Option<String>,
    },

    /// Manage sessions.
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
}

impl Cli {
    /// Effective runtime security for interactive chat sessions.
    pub fn resolve_runtime_security(&self, config: &Config) -> RuntimeSecurityConfig {
        let sandbox_mode = if let Some(explicit) = self.sandbox {
            explicit.into()
        } else if self.full_auto {
            SandboxMode::WorkspaceWrite
        } else if self.dangerously_bypass_approvals_and_sandbox {
            SandboxMode::DangerFullAccess
        } else {
            config.sandbox_mode
        };

        let approval_policy = if let Some(explicit) = self.ask_for_approval {
            explicit.into()
        } else if self.full_auto {
            ApprovalPolicy::OnRequest
        } else if self.dangerously_bypass_approvals_and_sandbox {
            ApprovalPolicy::Never
        } else {
            config.approval_policy
        };

        RuntimeSecurityConfig {
            approval_policy,
            sandbox_mode,
            sandbox_network_access: config.sandbox_network_access,
        }
    }

    /// Effective runtime security for non-interactive exec.
    pub fn resolve_runtime_security_for_exec(
        &self,
        config: &Config,
    ) -> anyhow::Result<RuntimeSecurityConfig> {
        let mut resolved = self.resolve_runtime_security(config);

        let explicit_prompting_policy = self.ask_for_approval.is_some_and(|p| {
            matches!(p, ApprovalPolicyArg::Untrusted | ApprovalPolicyArg::OnRequest)
        }) || self.full_auto;

        if explicit_prompting_policy {
            return Err(anyhow::anyhow!(
                "Non-interactive exec cannot use prompting approval policy. Use --ask-for-approval never (or --yolo)."
            ));
        }

        resolved.approval_policy = ApprovalPolicy::Never;
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::{ApprovalPolicyArg, Cli, Commands};
    use clap::Parser;
    use rot_core::{ApprovalPolicy, Config};

    #[test]
    fn test_exec_json_flags_conflict() {
        let parsed = Cli::try_parse_from([
            "rot",
            "exec",
            "hello",
            "--json",
            "--final-json",
        ]);
        assert!(parsed.is_err());
    }

    #[test]
    fn test_full_auto_conflicts_with_explicit_sandbox() {
        let parsed = Cli::try_parse_from([
            "rot",
            "--full-auto",
            "--sandbox",
            "read-only",
            "chat",
        ]);
        assert!(parsed.is_err());
    }

    #[test]
    fn test_yolo_alias_parses() {
        let parsed = Cli::try_parse_from(["rot", "--yolo", "chat"]).unwrap();
        assert!(parsed.dangerously_bypass_approvals_and_sandbox);
    }

    #[test]
    fn test_exec_command_parses_output_schema() {
        let parsed = Cli::try_parse_from([
            "rot",
            "exec",
            "hello",
            "--output-schema",
            "schema.json",
            "--json",
        ])
        .unwrap();

        match parsed.command {
            Some(Commands::Exec {
                json,
                final_json,
                output_schema,
                ..
            }) => {
                assert!(json);
                assert!(!final_json);
                assert_eq!(output_schema.as_deref(), Some("schema.json"));
            }
            _ => panic!("expected exec"),
        }
    }

    #[test]
    fn test_explicit_prompt_policy_detectable() {
        let parsed = Cli::try_parse_from([
            "rot",
            "--ask-for-approval",
            "on-request",
            "exec",
            "hello",
        ])
        .unwrap();
        assert_eq!(
            parsed.ask_for_approval,
            Some(ApprovalPolicyArg::OnRequest)
        );
    }

    #[test]
    fn test_exec_approval_forced_to_never_by_default() {
        let parsed = Cli::try_parse_from(["rot", "exec", "hello"]).unwrap();
        let cfg = Config::default();
        let resolved = parsed.resolve_runtime_security_for_exec(&cfg).unwrap();
        assert_eq!(resolved.approval_policy, ApprovalPolicy::Never);
    }

    #[test]
    fn test_exec_rejects_prompting_policy() {
        let parsed = Cli::try_parse_from([
            "rot",
            "--ask-for-approval",
            "on-request",
            "exec",
            "hello",
        ])
        .unwrap();
        let cfg = Config::default();
        assert!(parsed.resolve_runtime_security_for_exec(&cfg).is_err());
    }

    #[test]
    fn test_global_agent_flag_parses() {
        let parsed = Cli::try_parse_from(["rot", "--agent", "plan", "exec", "hello"]).unwrap();
        assert_eq!(parsed.agent.as_deref(), Some("plan"));
    }
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// List recent sessions.
    List {
        /// Maximum number of sessions to show.
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Resume a previous session.
    Resume {
        /// Session ID to resume.
        id: String,
    },
}
