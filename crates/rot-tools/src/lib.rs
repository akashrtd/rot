//! rot-tools: Built-in tools for files, shell, planning, and web operations.

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
    SandboxMode, SwarmExecution, SwarmRequest, SwarmWorkerResult, TaskExecution, TaskRequest,
    TaskRunner, Tool, ToolContext, ToolResult,
};
