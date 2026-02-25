//! rot-session: JSONL-based session persistence and management.

mod error;
pub mod format;
pub mod store;

pub use error::SessionError;
pub use format::{entry_id, entry_timestamp, SessionEntry, SessionMeta};
pub use store::{Session, SessionStore};
