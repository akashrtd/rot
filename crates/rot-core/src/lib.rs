//! rot-core: Core agent logic, message types, and permissions.

pub mod agent;
mod error;
pub mod message;
pub mod permission;
pub mod config;
pub mod security;


pub use agent::{Agent, AgentConfig, AgentProcessError};
pub use error::RotError;
pub use message::{ContentBlock, Message, MessageId, Role};
pub use config::{Config, ConfigStore};
pub use security::{ApprovalPolicy, RuntimeSecurityConfig, SandboxMode};
