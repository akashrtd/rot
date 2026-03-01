//! rot-core: Core agent logic, message types, and permissions.

pub mod agent;
pub mod agent_profile;
pub mod agent_registry;
mod error;
pub mod message;
pub mod permission;
pub mod config;
pub mod security;


pub use agent::{Agent, AgentConfig, AgentProcessError};
pub use agent_profile::{AgentMode, AgentProfile};
pub use agent_registry::{AgentRegistry, UnknownAgentError};
pub use error::RotError;
pub use message::{ContentBlock, Message, MessageId, Role};
pub use config::{Config, ConfigStore};
pub use security::{ApprovalPolicy, RuntimeSecurityConfig, SandboxMode};
