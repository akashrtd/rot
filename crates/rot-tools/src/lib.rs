//! rot-tools: Built-in tools (read, write, edit, bash, glob, grep, webfetch).

pub mod builtin;
mod external;
mod mcp;
mod error;
mod path_guard;
pub mod registry;
pub mod traits;

pub use builtin::register_all;
pub use error::ToolError;
pub use external::{register_custom_tools, CustomToolConfig};
pub use mcp::{register_mcp_tools, McpServerConfig};
pub use registry::ToolRegistry;
pub use traits::{
    SandboxMode, TaskExecution, TaskRequest, TaskRunner, Tool, ToolContext, ToolResult,
};
