//! rot-session: JSONL-based session persistence and management.

mod error;
pub mod format;

pub use error::SessionError;
pub use format::{SessionEntry, SessionMeta, entry_id, entry_timestamp};
