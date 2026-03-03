//! Context preflight, type detection, and extractor pipeline for RLM.

use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Normalized context payload after extraction.
#[derive(Debug, Clone)]
pub struct LoadedContext {
    /// Original source path passed by the user.
    pub source_path: PathBuf,
    /// Canonical extracted text artifact used by runtimes.
    pub extracted_path: PathBuf,
    /// Context type label.
    pub detected_type: String,
    /// Extracted text content.
    pub content: String,
}

impl LoadedContext {
    /// Extracted content length in bytes.
    pub fn extracted_length(&self) -> usize {
        self.content.len()
    }
}

/// Load and normalize context text from a source file.
pub async fn load_context(path: &Path) -> anyhow::Result<LoadedContext> {
    let source_path = path.canonicalize().map_err(|e| {
        anyhow::anyhow!(
            "Failed to resolve context path '{}': {e}",
            path.display()
        )
    })?;

    let bytes = tokio::fs::read(&source_path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read context '{}': {e}", source_path.display()))?;
    if bytes.is_empty() {
        return Err(anyhow::anyhow!("Context file is empty"));
    }

    let extension = source_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let (detected_type, content) = match extension.as_str() {
        "pdf" => ("pdf".to_string(), extract_pdf_text(&source_path).await?),
        "html" | "htm" => ("html".to_string(), extract_html_text(&bytes)?),
        "json" => ("json".to_string(), extract_json_text(&bytes)?),
        "csv" => ("csv".to_string(), extract_csv_text(&bytes)?),
        _ => {
            if looks_binary(&bytes) {
                return Err(anyhow::anyhow!(
                    "Unsupported binary context '{}'. Provide text/JSON/CSV/HTML/PDF input.",
                    source_path.display()
                ));
            }
            (
                "text".to_string(),
                String::from_utf8(bytes).map_err(|_| {
                    anyhow::anyhow!(
                        "Context '{}' is not valid UTF-8 text",
                        source_path.display()
                    )
                })?,
            )
        }
    };

    let cache_dir = std::env::temp_dir().join("rot-rlm").join("context-cache");
    tokio::fs::create_dir_all(&cache_dir).await.map_err(|e| {
        anyhow::anyhow!(
            "Failed to create context cache '{}': {e}",
            cache_dir.display()
        )
    })?;
    let cache_name = format!(
        "{}-{}.txt",
        blake3::hash(source_path.to_string_lossy().as_bytes()).to_hex(),
        ulid::Ulid::new()
    );
    let extracted_path = cache_dir.join(cache_name);
    tokio::fs::write(&extracted_path, &content).await.map_err(|e| {
        anyhow::anyhow!(
            "Failed to write extracted context '{}': {e}",
            extracted_path.display()
        )
    })?;

    Ok(LoadedContext {
        source_path,
        extracted_path,
        detected_type,
        content,
    })
}

fn looks_binary(bytes: &[u8]) -> bool {
    if bytes.contains(&0) {
        return true;
    }
    let sample = &bytes[..bytes.len().min(1024)];
    let non_text = sample
        .iter()
        .filter(|&&b| !(b == b'\n' || b == b'\r' || b == b'\t' || (32..=126).contains(&b)))
        .count();
    non_text * 100 / sample.len().max(1) > 20
}

async fn extract_pdf_text(path: &Path) -> anyhow::Result<String> {
    let cache_dir = std::env::temp_dir().join("rot-rlm").join("pdf");
    tokio::fs::create_dir_all(&cache_dir).await?;
    let out_path = cache_dir.join(format!("{}.txt", ulid::Ulid::new()));

    let output = Command::new("pdftotext")
        .arg(path)
        .arg(&out_path)
        .output()
        .await;

    match output {
        Ok(result) if result.status.success() => {
            let text = tokio::fs::read_to_string(&out_path).await.map_err(|e| {
                anyhow::anyhow!(
                    "pdftotext succeeded but output read failed '{}': {e}",
                    out_path.display()
                )
            })?;
            Ok(text)
        }
        Ok(result) => Err(anyhow::anyhow!(
            "Failed to extract PDF text from '{}': {}",
            path.display(),
            String::from_utf8_lossy(&result.stderr)
        )),
        Err(_) => Err(anyhow::anyhow!(
            "PDF context requires `pdftotext` on PATH. Install poppler and retry."
        )),
    }
}

fn extract_html_text(bytes: &[u8]) -> anyhow::Result<String> {
    let raw = String::from_utf8(bytes.to_vec())
        .map_err(|_| anyhow::anyhow!("HTML context is not valid UTF-8"))?;
    let no_script = regex::Regex::new("(?is)<script.*?>.*?</script>")
        .unwrap()
        .replace_all(&raw, " ");
    let no_style = regex::Regex::new("(?is)<style.*?>.*?</style>")
        .unwrap()
        .replace_all(&no_script, " ");
    let no_tags = regex::Regex::new("(?is)<[^>]+>")
        .unwrap()
        .replace_all(&no_style, " ");
    let cleaned = no_tags
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(cleaned)
}

fn extract_json_text(bytes: &[u8]) -> anyhow::Result<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| anyhow::anyhow!("Malformed JSON context: {e}"))?;
    serde_json::to_string_pretty(&value)
        .map_err(|e| anyhow::anyhow!("Failed to render JSON context: {e}"))
}

fn extract_csv_text(bytes: &[u8]) -> anyhow::Result<String> {
    let raw = String::from_utf8(bytes.to_vec())
        .map_err(|_| anyhow::anyhow!("CSV context is not valid UTF-8"))?;
    let lines = raw
        .lines()
        .map(|line| line.trim_end())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::load_context;
    use std::path::Path;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_text_context_loads() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ctx.txt");
        std::fs::write(&path, "hello\nworld\n").unwrap();

        let loaded = load_context(Path::new(&path)).await.unwrap();
        assert_eq!(loaded.detected_type, "text");
        assert_eq!(loaded.content, "hello\nworld\n");
        assert!(loaded.extracted_path.exists());
    }

    #[tokio::test]
    async fn test_binary_context_rejected() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ctx.bin");
        std::fs::write(&path, vec![0_u8, 1, 2, 3, 4]).unwrap();

        let err = load_context(Path::new(&path)).await.unwrap_err();
        assert!(err.to_string().contains("Unsupported binary context"));
    }

    #[tokio::test]
    async fn test_json_context_pretty_printed() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ctx.json");
        std::fs::write(&path, r#"{"a":1,"b":{"c":2}}"#).unwrap();

        let loaded = load_context(Path::new(&path)).await.unwrap();
        assert_eq!(loaded.detected_type, "json");
        assert!(loaded.content.contains("\n"));
        assert!(loaded.content.contains("\"a\""));
    }

    #[tokio::test]
    async fn test_pdf_missing_extractor_is_actionable() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ctx.pdf");
        std::fs::write(&path, b"%PDF-1.4").unwrap();

        // The file isn't a valid PDF, but this should still produce an actionable
        // extractor-level error rather than a UTF-8 read failure.
        let err = load_context(Path::new(&path)).await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("pdftotext") || msg.contains("Failed to extract PDF text"));
    }
}
