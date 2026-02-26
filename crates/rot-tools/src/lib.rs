//! rot-tools: Built-in tools (read, write, edit, bash, glob, grep, webfetch).

pub mod builtin;
mod error;
mod path_guard;
pub mod registry;
pub mod traits;

pub use builtin::register_all;
pub use error::ToolError;
pub use registry::ToolRegistry;
pub use traits::{SandboxMode, Tool, ToolContext, ToolResult};
