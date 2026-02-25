//! rot-tools: Built-in tools (read, write, edit, bash, glob, grep, webfetch).

mod error;
pub mod registry;
pub mod traits;

pub use error::ToolError;
pub use registry::ToolRegistry;
pub use traits::{Tool, ToolContext, ToolResult};
