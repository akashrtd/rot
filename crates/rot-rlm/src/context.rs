//! RLM (Recursive Language Model) context management.
//!
//! The ContextManager handles storing large content in temporary files
//! and building metadata summaries for the LLM to reference.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Manages external context storage for RLM sessions.
///
/// When content is too large for the context window, it gets stored
/// in temporary files and the LLM receives a variable reference instead.
pub struct ContextManager {
    temp_dir: PathBuf,
    vars: HashMap<String, PathBuf>,
    counter: usize,
}

impl ContextManager {
    /// Create a new context manager.
    pub async fn new() -> std::io::Result<Self> {
        let temp_dir = std::env::temp_dir().join("rot-rlm");
        fs::create_dir_all(&temp_dir).await?;
        Ok(Self {
            temp_dir,
            vars: HashMap::new(),
            counter: 0,
        })
    }

    /// Create with a specific directory (for testing).
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            temp_dir: dir.into(),
            vars: HashMap::new(),
            counter: 0,
        }
    }

    /// Store content in external storage, returning a variable name.
    pub async fn store(&mut self, content: &str) -> std::io::Result<String> {
        self.counter += 1;
        let var_name = format!("ctx_{}", self.counter);
        let file_path = self.temp_dir.join(format!("{var_name}.txt"));

        fs::write(&file_path, content).await?;
        self.vars.insert(var_name.clone(), file_path);

        Ok(var_name)
    }

    /// Load content by variable name.
    pub async fn load(&self, var_name: &str) -> std::io::Result<String> {
        let path = self.vars.get(var_name).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, format!("Unknown var: {var_name}"))
        })?;
        fs::read_to_string(path).await
    }

    /// Build a metadata summary for a piece of content.
    ///
    /// Returns a brief description including line count and byte size,
    /// suitable for injecting into the LLM context instead of the full content.
    pub fn build_metadata(content: &str) -> String {
        let lines = content.lines().count();
        let bytes = content.len();
        let preview: String = content.chars().take(200).collect();
        let preview = preview.replace('\n', "\\n");

        format!(
            "[{lines} lines, {bytes} bytes]\nPreview: {preview}{}",
            if content.len() > 200 { "..." } else { "" }
        )
    }

    /// Get the temp directory path.
    pub fn temp_dir(&self) -> &Path {
        &self.temp_dir
    }

    /// List all stored variable names.
    pub fn vars(&self) -> Vec<&str> {
        self.vars.keys().map(|s| s.as_str()).collect()
    }

    /// Clean up stored files.
    pub async fn cleanup(&self) -> std::io::Result<()> {
        for path in self.vars.values() {
            let _ = fs::remove_file(path).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_store_and_load() {
        let dir = TempDir::new().unwrap();
        let mut cm = ContextManager::with_dir(dir.path());

        let var = cm.store("hello world").await.unwrap();
        assert_eq!(var, "ctx_1");

        let content = cm.load(&var).await.unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_store_large_content() {
        let dir = TempDir::new().unwrap();
        let mut cm = ContextManager::with_dir(dir.path());

        let large = "x".repeat(100_000);
        let var = cm.store(&large).await.unwrap();
        let loaded = cm.load(&var).await.unwrap();
        assert_eq!(loaded.len(), 100_000);
    }

    #[tokio::test]
    async fn test_metadata_generation() {
        let content = "fn main() {\n    println!(\"Hello, world!\");\n}\n";
        let meta = ContextManager::build_metadata(content);
        assert!(meta.contains("3 lines"));
        assert!(meta.contains("bytes"));
        assert!(meta.contains("fn main"));
    }

    #[tokio::test]
    async fn test_load_unknown_var() {
        let dir = TempDir::new().unwrap();
        let cm = ContextManager::with_dir(dir.path());
        assert!(cm.load("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn test_multiple_vars() {
        let dir = TempDir::new().unwrap();
        let mut cm = ContextManager::with_dir(dir.path());

        let v1 = cm.store("first").await.unwrap();
        let v2 = cm.store("second").await.unwrap();

        assert_ne!(v1, v2);
        assert_eq!(cm.vars().len(), 2);
    }
}
