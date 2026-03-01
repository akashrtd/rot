//! JSONL session store implementation.

use crate::error::SessionError;
use crate::format::{entry_timestamp, SessionEntry, SessionMeta, SessionTree, SessionTreeNode};
use std::collections::HashMap;
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
        self.create_with_parent(cwd, model, provider, None, None, None)
            .await
    }

    /// Create a new child session linked to a parent session.
    pub async fn create_child(
        &self,
        cwd: &Path,
        model: &str,
        provider: &str,
        parent_session_id: &str,
        parent_tool_call_id: Option<&str>,
        agent: Option<&str>,
    ) -> Result<Session, SessionError> {
        self.create_with_parent(
            cwd,
            model,
            provider,
            Some(parent_session_id),
            parent_tool_call_id,
            agent,
        )
        .await
    }

    async fn create_with_parent(
        &self,
        cwd: &Path,
        model: &str,
        provider: &str,
        parent_session_id: Option<&str>,
        parent_tool_call_id: Option<&str>,
        agent: Option<&str>,
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
            parent_session_id: parent_session_id.map(str::to_string),
            parent_tool_call_id: parent_tool_call_id.map(str::to_string),
            agent: agent.map(str::to_string),
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

    /// Append an entry to an existing session identified by ID.
    pub async fn append_by_id(
        &self,
        cwd: &Path,
        session_id: &str,
        entry: SessionEntry,
    ) -> Result<(), SessionError> {
        let mut session = self.load(cwd, session_id).await?;
        self.append(&mut session, entry).await
    }

    /// List recent sessions for a working directory.
    pub async fn list_recent(
        &self,
        cwd: &Path,
        limit: usize,
    ) -> Result<Vec<SessionMeta>, SessionError> {
        let mut sessions = self.list_all(cwd).await?;
        sessions.truncate(limit);
        Ok(sessions)
    }

    /// List all sessions for a working directory, most recent first.
    pub async fn list_all(&self, cwd: &Path) -> Result<Vec<SessionMeta>, SessionError> {
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
        Ok(sessions)
    }

    /// Build the parent/child session tree for a session or the latest session in the cwd.
    pub async fn tree(
        &self,
        cwd: &Path,
        session_id: Option<&str>,
    ) -> Result<SessionTree, SessionError> {
        let sessions = self.list_all(cwd).await?;
        if sessions.is_empty() {
            return Err(SessionError::NotFound(
                session_id.unwrap_or("latest").to_string(),
            ));
        }

        let focus_id = session_id
            .map(str::to_string)
            .unwrap_or_else(|| sessions[0].id.clone());

        let metas_by_id = sessions
            .into_iter()
            .map(|meta| (meta.id.clone(), meta))
            .collect::<HashMap<_, _>>();

        let Some(focus_meta) = metas_by_id.get(&focus_id) else {
            return Err(SessionError::NotFound(focus_id));
        };

        let mut root_id = focus_meta.id.clone();
        while let Some(parent_id) = metas_by_id
            .get(&root_id)
            .and_then(|meta| meta.parent_session_id.clone())
        {
            root_id = parent_id;
        }

        let mut children_by_parent: HashMap<String, Vec<String>> = HashMap::new();
        for meta in metas_by_id.values() {
            if let Some(parent_id) = &meta.parent_session_id {
                children_by_parent
                    .entry(parent_id.clone())
                    .or_default()
                    .push(meta.id.clone());
            }
        }

        for child_ids in children_by_parent.values_mut() {
            child_ids.sort_by(|left, right| {
                let left_meta = metas_by_id.get(left).expect("child session missing");
                let right_meta = metas_by_id.get(right).expect("child session missing");
                left_meta.created_at.cmp(&right_meta.created_at)
            });
        }

        let root = build_tree_node(&root_id, &metas_by_id, &children_by_parent)?;
        Ok(SessionTree { root, focus_id })
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
                parent_session_id,
                agent,
                ..
            } => Ok(SessionMeta {
                id,
                created_at: timestamp,
                updated_at: entry_timestamp(&last),
                title: None,
                cwd,
                model,
                provider,
                parent_session_id,
                agent,
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

fn build_tree_node(
    id: &str,
    metas_by_id: &HashMap<String, SessionMeta>,
    children_by_parent: &HashMap<String, Vec<String>>,
) -> Result<SessionTreeNode, SessionError> {
    let Some(meta) = metas_by_id.get(id) else {
        return Err(SessionError::InvalidFormat(format!(
            "Unknown session referenced in tree: {}",
            id
        )));
    };

    let mut children = Vec::new();
    if let Some(child_ids) = children_by_parent.get(id) {
        for child_id in child_ids {
            children.push(build_tree_node(child_id, metas_by_id, children_by_parent)?);
        }
    }

    Ok(SessionTreeNode {
        meta: meta.clone(),
        children,
    })
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
    async fn test_create_child_session() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_dir(dir.path());
        let cwd = dir.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        let child = store
            .create_child(
                &cwd,
                "claude",
                "anthropic",
                "parent-session",
                Some("tool-call-1"),
                Some("review"),
            )
            .await
            .unwrap();

        match &child.entries[0] {
            SessionEntry::SessionStart {
                parent_session_id,
                parent_tool_call_id,
                agent,
                ..
            } => {
                assert_eq!(parent_session_id.as_deref(), Some("parent-session"));
                assert_eq!(parent_tool_call_id.as_deref(), Some("tool-call-1"));
                assert_eq!(agent.as_deref(), Some("review"));
            }
            other => panic!("expected session start, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_append_by_id() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_dir(dir.path());
        let cwd = dir.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        let session = store.create(&cwd, "claude", "anthropic").await.unwrap();

        store
            .append_by_id(
                &cwd,
                &session.id,
                SessionEntry::ChildSessionLink {
                    id: "link-1".to_string(),
                    parent_session_id: session.id.clone(),
                    child_session_id: "child-1".to_string(),
                    timestamp: 1000,
                    agent: "review".to_string(),
                    prompt: "inspect changes".to_string(),
                },
            )
            .await
            .unwrap();

        let loaded = store.load(&cwd, &session.id).await.unwrap();
        assert_eq!(loaded.entries.len(), 2);
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
    async fn test_build_session_tree_from_child_focus() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_dir(dir.path());
        let cwd = dir.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        let root = store.create(&cwd, "claude", "anthropic").await.unwrap();
        let child = store
            .create_child(&cwd, "claude", "anthropic", &root.id, None, Some("review"))
            .await
            .unwrap();
        let grandchild = store
            .create_child(&cwd, "claude", "anthropic", &child.id, None, Some("explore"))
            .await
            .unwrap();

        let tree = store.tree(&cwd, Some(&grandchild.id)).await.unwrap();
        assert_eq!(tree.focus_id, grandchild.id);
        assert_eq!(tree.root.meta.id, root.id);
        assert_eq!(tree.root.children.len(), 1);
        assert_eq!(tree.root.children[0].meta.id, child.id);
        assert_eq!(tree.root.children[0].children[0].meta.id, grandchild.id);
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_dir(dir.path());
        let result = store.load(dir.path(), "nonexistent").await;
        assert!(result.is_err());
    }
}
