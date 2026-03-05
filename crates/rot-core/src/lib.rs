//! rot-core: Core agent logic, message types, and permissions.

pub mod agent;
pub mod agent_profile;
pub mod agent_registry;
pub mod error;
pub mod message;
pub mod permission;
pub mod config;
pub mod security;
pub mod swarm;
pub mod task_scheduler;


pub use agent::{Agent, AgentConfig, TaskExecutionPolicy};
pub use agent_profile::{AgentMode, AgentProfile};
pub use agent_registry::{AgentRegistry, UnknownAgentError};
pub use error::{RotError, AgentError};
pub use message::{ContentBlock, Message, MessageId, Role};
pub use config::{Config, ConfigStore};
pub use security::{ApprovalPolicy, RuntimeSecurityConfig, SandboxMode};
