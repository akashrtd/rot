//! rot-core: Core agent logic, message types, and permissions.

pub mod agent;
mod error;
pub mod message;
pub mod permission;


pub use agent::{Agent, AgentConfig, AgentProcessError};
pub use error::RotError;
pub use message::{ContentBlock, Message, MessageId, Role};
