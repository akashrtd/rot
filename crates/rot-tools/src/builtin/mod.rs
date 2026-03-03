//! Built-in tool implementations.

pub mod bash;
pub mod codesearch;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod list;
pub mod lsp;
pub mod patch;
pub mod question;
pub mod read;
pub mod task;
pub mod todoread;
pub mod todowrite;
pub mod webfetch;
pub mod websearch;
pub mod write;
mod todostate;

use crate::ToolRegistry;
use std::sync::Arc;

/// Register all built-in tools into a registry.
pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Arc::new(read::ReadTool));
    registry.register(Arc::new(list::ListTool));
    registry.register(Arc::new(codesearch::CodeSearchTool));
    registry.register(Arc::new(lsp::LspTool));
    registry.register(Arc::new(write::WriteTool));
    registry.register(Arc::new(edit::EditTool));
    registry.register(Arc::new(patch::PatchTool));
    registry.register(Arc::new(question::QuestionTool));
    registry.register(Arc::new(todoread::TodoReadTool));
    registry.register(Arc::new(todowrite::TodoWriteTool));
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(glob::GlobTool));
    registry.register(Arc::new(grep::GrepTool));
    registry.register(Arc::new(task::TaskTool));
    registry.register(Arc::new(webfetch::WebFetchTool));
    registry.register(Arc::new(websearch::WebSearchTool));
}
