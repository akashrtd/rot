pub mod context;
pub mod context_loader;
pub mod config;
pub mod engine;
pub mod prompts;
pub mod python_repl;
pub mod repl;
pub mod runtime;
pub mod runtime_docker;
pub mod runtime_local;
pub mod subcall;
pub mod trace;
pub mod usage;

pub use context::*;
pub use context_loader::{LoadedContext, load_context};
pub use config::RlmConfig;
pub use engine::{RlmEngine, RlmRunReport};
pub use prompts::RLM_SYSTEM_PROMPT;
pub use python_repl::PythonReplEnv;
pub use repl::{ReplEnv, ReplResult};
pub use runtime::{RlmIsolationKind, RlmProcessPolicy, RlmRuntimeKind};
pub use subcall::{SUBLM_MARKER, SubcallRecord, SubcallRequest};
pub use trace::{ExecutionTrace, IterationTrace, RlmTrajectory};
pub use usage::RlmUsage;

#[cfg(test)]
mod engine_tests;
