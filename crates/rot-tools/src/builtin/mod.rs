//! Built-in tool implementations.

pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod read;
pub mod webfetch;
pub mod write;

use crate::ToolRegistry;
use std::sync::Arc;

/// Register all built-in tools into a registry.
pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Arc::new(read::ReadTool));
    registry.register(Arc::new(write::WriteTool));
    registry.register(Arc::new(edit::EditTool));
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(glob::GlobTool));
    registry.register(Arc::new(grep::GrepTool));
    registry.register(Arc::new(webfetch::WebFetchTool));
}
