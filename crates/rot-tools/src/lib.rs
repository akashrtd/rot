//! rot-tools: Built-in tools (read, write, edit, bash, glob, grep, webfetch).

pub mod builtin;
mod error;
pub mod registry;
pub mod traits;

pub use builtin::register_all;
pub use error::ToolError;
pub use registry::ToolRegistry;
pub use traits::{Tool, ToolContext, ToolResult};
