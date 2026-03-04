//! CLI argument and command definitions.

use clap::{Parser, Subcommand, ValueEnum};
use rot_core::config::Config;
use rot_core::security::{ApprovalPolicy, RuntimeSecurityConfig, SandboxMode};
use rot_rlm::{RlmIsolationKind, RlmRuntimeKind};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RlmRuntimeArg {
    Python,
    Bash,
}

impl From<RlmRuntimeArg> for RlmRuntimeKind {
    fn from(value: RlmRuntimeArg) -> Self {
        match value {
            RlmRuntimeArg::Python => RlmRuntimeKind::Python,
            RlmRuntimeArg::Bash => RlmRuntimeKind::Bash,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RlmIsolationArg {
    Local,
    Docker,
}

impl From<RlmIsolationArg> for RlmIsolationKind {
    fn from(value: RlmIsolationArg) -> Self {
        match value {
            RlmIsolationArg::Local => RlmIsolationKind::Local,
            RlmIsolationArg::Docker => RlmIsolationKind::Docker,
        }
    }
}

#[derive(Parser)]
#[command(name = "rot", version, about = "Recursive Operations Tool — AI coding agent")]
pub struct Cli {
    /// LLM provider to use.
    #[arg(long, global = true)]
    pub provider: Option<String>,

    /// Built-in agent profile to use.
    #[arg(long, global = true)]
    pub agent: Option<String>,

    /// Model to use (defaults to provider's default model).
    #[arg(long, global = true)]
    pub model: Option<String>,

    /// Enable verbose logging.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Explicitly allow RLM execution in danger-full-access mode.
    #[arg(long, global = true)]
    pub allow_unsafe_rlm: bool,

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

        /// Resume from an existing session ID.
        #[arg(long)]
        session: Option<String>,

        /// Fork into a child session from --session.
        #[arg(long, requires = "session")]
        fork: bool,

        /// Run using the Recursive Language Model (RLM) engine for huge contexts.
        #[arg(long)]
        rlm: bool,

        /// External context file to map into the RLM environment (required if --rlm is used)
        #[arg(long, requires = "rlm")]
        context: Option<String>,

        /// Runtime backend for RLM execution.
        #[arg(long = "rlm-runtime", requires = "rlm", value_enum)]
        rlm_runtime: Option<RlmRuntimeArg>,

        /// Process isolation backend for RLM runtime subprocesses.
        #[arg(long = "rlm-isolation", requires = "rlm", value_enum)]
        rlm_isolation: Option<RlmIsolationArg>,

        /// Docker image used when --rlm-isolation=docker.
        #[arg(long = "rlm-docker-image", requires = "rlm")]
        rlm_docker_image: Option<String>,

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

    /// Inspect loaded tools.
    Tools {
        /// Show one tool in detail.
        name: Option<String>,
    },

    /// List configured and available providers.
    Providers,

    /// List models for the active provider.
    Models,

    /// Run a local HTTP service for headless exec automation.
    Serve {
        /// Host interface to bind.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port to bind.
        #[arg(long, default_value = "7878")]
        port: u16,
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
    use super::{ApprovalPolicyArg, Cli, Commands, SessionAction};
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
                session,
                fork,
                ..
            }) => {
                assert!(json);
                assert!(!final_json);
                assert_eq!(output_schema.as_deref(), Some("schema.json"));
                assert!(session.is_none());
                assert!(!fork);
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

    #[test]
    fn test_session_tree_command_parses() {
        let parsed = Cli::try_parse_from(["rot", "session", "tree", "abc123"]).unwrap();
        match parsed.command {
            Some(Commands::Session {
                action: SessionAction::Tree { id },
            }) => assert_eq!(id.as_deref(), Some("abc123")),
            _ => panic!("expected session tree command"),
        }
    }

    #[test]
    fn test_tools_command_parses_optional_name() {
        let parsed = Cli::try_parse_from(["rot", "tools", "read"]).unwrap();
        match parsed.command {
            Some(Commands::Tools { name }) => assert_eq!(name.as_deref(), Some("read")),
            _ => panic!("expected tools command"),
        }
    }

    #[test]
    fn test_providers_command_parses() {
        let parsed = Cli::try_parse_from(["rot", "providers"]).unwrap();
        assert!(matches!(parsed.command, Some(Commands::Providers)));
    }

    #[test]
    fn test_models_command_parses() {
        let parsed = Cli::try_parse_from(["rot", "models"]).unwrap();
        assert!(matches!(parsed.command, Some(Commands::Models)));
    }

    #[test]
    fn test_provider_is_unset_by_default() {
        let parsed = Cli::try_parse_from(["rot", "models"]).unwrap();
        assert!(parsed.provider.is_none());
    }

    #[test]
    fn test_provider_flag_parses() {
        let parsed = Cli::try_parse_from(["rot", "--provider", "openai", "models"]).unwrap();
        assert_eq!(parsed.provider.as_deref(), Some("openai"));
    }

    #[test]
    fn test_serve_command_parses() {
        let parsed = Cli::try_parse_from(["rot", "serve", "--host", "0.0.0.0", "--port", "9000"])
            .unwrap();
        match parsed.command {
            Some(Commands::Serve { host, port }) => {
                assert_eq!(host, "0.0.0.0");
                assert_eq!(port, 9000);
            }
            _ => panic!("expected serve"),
        }
    }

    #[test]
    fn test_exec_rlm_runtime_parses() {
        let parsed = Cli::try_parse_from([
            "rot",
            "exec",
            "analyze",
            "--rlm",
            "--context",
            "ctx.txt",
            "--rlm-runtime",
            "bash",
        ])
        .unwrap();
        match parsed.command {
            Some(Commands::Exec { rlm_runtime, .. }) => {
                assert!(matches!(rlm_runtime, Some(super::RlmRuntimeArg::Bash)));
            }
            _ => panic!("expected exec"),
        }
    }

    #[test]
    fn test_exec_rlm_isolation_parses() {
        let parsed = Cli::try_parse_from([
            "rot",
            "exec",
            "analyze",
            "--rlm",
            "--context",
            "ctx.txt",
            "--rlm-isolation",
            "docker",
            "--rlm-docker-image",
            "python:3.11-slim",
        ])
        .unwrap();
        match parsed.command {
            Some(Commands::Exec {
                rlm_isolation,
                rlm_docker_image,
                ..
            }) => {
                assert!(matches!(
                    rlm_isolation,
                    Some(super::RlmIsolationArg::Docker)
                ));
                assert_eq!(rlm_docker_image.as_deref(), Some("python:3.11-slim"));
            }
            _ => panic!("expected exec"),
        }
    }

    #[test]
    fn test_exec_session_fork_parses() {
        let parsed = Cli::try_parse_from([
            "rot",
            "exec",
            "hello",
            "--session",
            "abc123",
            "--fork",
        ])
        .unwrap();
        match parsed.command {
            Some(Commands::Exec { session, fork, .. }) => {
                assert_eq!(session.as_deref(), Some("abc123"));
                assert!(fork);
            }
            _ => panic!("expected exec"),
        }
    }

    #[test]
    fn test_session_export_import_parse() {
        let export = Cli::try_parse_from([
            "rot",
            "session",
            "export",
            "abc123",
            "out.jsonl",
        ])
        .unwrap();
        assert!(matches!(
            export.command,
            Some(Commands::Session {
                action: SessionAction::Export { .. }
            })
        ));

        let import = Cli::try_parse_from([
            "rot",
            "session",
            "import",
            "in.jsonl",
            "--id",
            "abc123",
        ])
        .unwrap();
        assert!(matches!(
            import.command,
            Some(Commands::Session {
                action: SessionAction::Import { .. }
            })
        ));
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
    /// Show the parent/child tree for a session or the latest session.
    Tree {
        /// Session ID to focus in the rendered tree. Defaults to the latest session.
        id: Option<String>,
    },
    /// Resume a previous session.
    Resume {
        /// Session ID to resume.
        id: String,
    },
    /// Export a session JSONL transcript.
    Export {
        /// Session ID to export.
        id: String,
        /// Output JSONL file path.
        output: String,
    },
    /// Import a session JSONL transcript.
    Import {
        /// Input JSONL file path.
        input: String,
        /// Optional session ID override.
        #[arg(long)]
        id: Option<String>,
    },
}
