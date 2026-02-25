//! JSONL session store implementation.

use crate::error::SessionError;
use crate::format::{entry_timestamp, SessionEntry, SessionMeta};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Active session handle.
#[derive(Debug)]
pub struct Session {
    /// Session identifier.
    pub id: String,
    /// Path to the JSONL file.
    pub file_path: PathBuf,
    /// Working directory for this session.
    pub cwd: PathBuf,
    /// Loaded entries.
    pub entries: Vec<SessionEntry>,
    /// ID of the current leaf (most recent message).
    pub current_leaf: String,
}

/// Persistent session storage.
pub struct SessionStore {
    sessions_dir: PathBuf,
}

impl SessionStore {
    /// Create a new session store. Uses `~/.local/share/rot/sessions/` by default.
    pub fn new() -> Self {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rot")
            .join("sessions");
        Self {
            sessions_dir: base,
        }
    }

    /// Create with a custom directory (for testing).
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            sessions_dir: dir.into(),
        }
    }

    /// Hash a working directory into a short folder name.
    fn cwd_hash(cwd: &Path) -> String {
        let hash = blake3::hash(cwd.to_string_lossy().as_bytes());
        hash.to_hex()[..16].to_string()
    }

    /// Get the session file path.
    fn session_path(&self, cwd: &Path, id: &str) -> PathBuf {
        self.sessions_dir
            .join(Self::cwd_hash(cwd))
            .join(format!("{id}.jsonl"))
    }

    /// Create a new session.
    pub async fn create(
        &self,
        cwd: &Path,
        model: &str,
        provider: &str,
    ) -> Result<Session, SessionError> {
        let id = ulid::Ulid::new().to_string();
        let file_path = self.session_path(cwd, &id);

        // Ensure directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let start_entry = SessionEntry::SessionStart {
            id: id.clone(),
            timestamp,
            cwd: cwd.to_string_lossy().to_string(),
            model: model.to_string(),
            provider: provider.to_string(),
        };

        // Write start entry
        let line = serde_json::to_string(&start_entry)?;
        fs::write(&file_path, format!("{line}\n")).await?;

        Ok(Session {
            id: id.clone(),
            file_path,
            cwd: cwd.to_path_buf(),
            entries: vec![start_entry],
            current_leaf: id,
        })
    }

    /// Load an existing session by ID.
    pub async fn load(&self, cwd: &Path, id: &str) -> Result<Session, SessionError> {
        let file_path = self.session_path(cwd, id);

        if !file_path.exists() {
            return Err(SessionError::NotFound(id.to_string()));
        }

        let content = fs::read_to_string(&file_path).await?;
        let mut entries = Vec::new();
        let mut current_leaf = id.to_string();

        for (line_num, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: SessionEntry = serde_json::from_str(line).map_err(|e| {
                SessionError::InvalidFormat(format!("Line {}: {e}", line_num + 1))
            })?;
            if let SessionEntry::Message { ref id, .. } = entry {
                current_leaf = id.clone();
            }
            entries.push(entry);
        }

        Ok(Session {
            id: id.to_string(),
            file_path,
            cwd: cwd.to_path_buf(),
            entries,
            current_leaf,
        })
    }

    /// Append an entry to a session.
    pub async fn append(
        &self,
        session: &mut Session,
        entry: SessionEntry,
    ) -> Result<(), SessionError> {
        let line = serde_json::to_string(&entry)?;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&session.file_path)
            .await?;
        file.write_all(format!("{line}\n").as_bytes()).await?;

        // Update current_leaf
        if let SessionEntry::Message { ref id, .. } = entry {
            session.current_leaf = id.clone();
        }
        session.entries.push(entry);

        Ok(())
    }

    /// List recent sessions for a working directory.
    pub async fn list_recent(
        &self,
        cwd: &Path,
        limit: usize,
    ) -> Result<Vec<SessionMeta>, SessionError> {
        let dir = self.sessions_dir.join(Self::cwd_hash(cwd));
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(&dir).await?;
        let mut sessions = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                if let Ok(meta) = self.read_session_meta(&path).await {
                    sessions.push(meta);
                }
            }
        }

        // Sort by most recent first
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions.truncate(limit);
        Ok(sessions)
    }

    /// Read metadata from a session file (parses first and last lines).
    async fn read_session_meta(&self, path: &Path) -> Result<SessionMeta, SessionError> {
        let content = fs::read_to_string(path).await?;
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return Err(SessionError::InvalidFormat("Empty session file".to_string()));
        }

        let first: SessionEntry = serde_json::from_str(lines[0])?;
        let last: SessionEntry = if lines.len() > 1 {
            serde_json::from_str(lines[lines.len() - 1])?
        } else {
            serde_json::from_str(lines[0])?
        };

        let message_count = lines
            .iter()
            .filter(|l| l.contains("\"type\":\"message\""))
            .count();

        match first {
            SessionEntry::SessionStart {
                id,
                timestamp,
                cwd,
                model,
                provider,
            } => Ok(SessionMeta {
                id,
                created_at: timestamp,
                updated_at: entry_timestamp(&last),
                title: None,
                cwd,
                model,
                provider,
                message_count,
            }),
            _ => Err(SessionError::InvalidFormat(
                "First entry is not SessionStart".to_string(),
            )),
        }
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_session() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_dir(dir.path());
        let cwd = dir.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        let session = store.create(&cwd, "claude-sonnet-4-20250514", "anthropic").await.unwrap();
        assert!(!session.id.is_empty());
        assert!(session.file_path.exists());
        assert_eq!(session.entries.len(), 1);
    }

    #[tokio::test]
    async fn test_append_and_load() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_dir(dir.path());
        let cwd = dir.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        let mut session = store.create(&cwd, "claude", "anthropic").await.unwrap();
        let session_id = session.id.clone();

        store
            .append(
                &mut session,
                SessionEntry::Message {
                    id: "msg1".to_string(),
                    parent_id: None,
                    timestamp: 1000,
                    role: "user".to_string(),
                    content: serde_json::json!([{"type": "text", "text": "Hello"}]),
                },
            )
            .await
            .unwrap();

        assert_eq!(session.entries.len(), 2);
        assert_eq!(session.current_leaf, "msg1");

        // Reload from disk
        let loaded = store.load(&cwd, &session_id).await.unwrap();
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.current_leaf, "msg1");
    }

    #[tokio::test]
    async fn test_list_recent() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_dir(dir.path());
        let cwd = dir.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        // Create 3 sessions
        store.create(&cwd, "claude", "anthropic").await.unwrap();
        store.create(&cwd, "gpt-4", "openai").await.unwrap();
        store.create(&cwd, "claude", "anthropic").await.unwrap();

        let sessions = store.list_recent(&cwd, 10).await.unwrap();
        assert_eq!(sessions.len(), 3);
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_dir(dir.path());
        let result = store.load(dir.path(), "nonexistent").await;
        assert!(result.is_err());
    }
}
