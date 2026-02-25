//! rot-core: Core agent logic, message types, and permissions.

mod error;
pub mod message;

pub use error::RotError;
pub use message::{ContentBlock, Message, MessageId, Role};
