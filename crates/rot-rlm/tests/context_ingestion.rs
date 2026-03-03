use rot_rlm::load_context;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn path_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct PathOverride {
    previous: Option<OsString>,
}

impl Drop for PathOverride {
    fn drop(&mut self) {
        if let Some(prev) = self.previous.take() {
            std::env::set_var("PATH", prev);
        } else {
            std::env::remove_var("PATH");
        }
    }
}

fn override_path(path: &Path) -> PathOverride {
    let previous = std::env::var_os("PATH");
    std::env::set_var("PATH", path.as_os_str());
    PathOverride { previous }
}

#[cfg(unix)]
fn create_fake_pdftotext(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let bin = dir.join("pdftotext");
    std::fs::write(
        &bin,
        "#!/bin/sh\nout=\"$2\"\nprintf 'fixture extracted pdf text\\n' > \"$out\"\n",
    )
    .unwrap();
    let mut perms = std::fs::metadata(&bin).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&bin, perms).unwrap();
    bin
}

#[cfg(windows)]
fn create_fake_pdftotext(dir: &Path) -> PathBuf {
    let bin = dir.join("pdftotext.bat");
    std::fs::write(
        &bin,
        "@echo off\r\nset OUT=%2\r\necho fixture extracted pdf text>%OUT%\r\n",
    )
    .unwrap();
    bin
}

#[tokio::test]
async fn test_plain_text_context_fixture() {
    let loaded = load_context(&fixture("sample.txt")).await.unwrap();
    assert_eq!(loaded.detected_type, "text");
    assert!(loaded.content.contains("RLM fixture text"));
    assert!(loaded.extracted_path.exists());
}

#[tokio::test]
async fn test_binary_file_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("sample.bin");
    std::fs::write(&bin, [0_u8, 159, 146, 150, 0, 42]).unwrap();

    let err = load_context(&bin).await.unwrap_err().to_string();
    assert!(err.contains("Unsupported binary context"));
}

#[tokio::test]
async fn test_pdf_preprocessing_success_with_extractor() {
    let _guard = path_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let _fake = create_fake_pdftotext(dir.path());
    let _path_override = override_path(dir.path());

    let loaded = load_context(&fixture("sample.pdf")).await.unwrap();
    assert_eq!(loaded.detected_type, "pdf");
    assert!(loaded.content.contains("fixture extracted pdf text"));
}

#[tokio::test]
async fn test_pdf_extractor_unavailable_path() {
    let _guard = path_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let _path_override = override_path(dir.path());

    let err = load_context(&fixture("sample.pdf")).await.unwrap_err().to_string();
    assert!(err.contains("requires `pdftotext`"));
}

#[tokio::test]
async fn test_malformed_json_fails_and_csv_fallback_loads() {
    let json_err = load_context(&fixture("malformed.json"))
        .await
        .unwrap_err()
        .to_string();
    assert!(json_err.contains("Malformed JSON context"));

    let csv_loaded = load_context(&fixture("malformed.csv")).await.unwrap();
    assert_eq!(csv_loaded.detected_type, "csv");
    assert!(csv_loaded.content.contains("\"alice,30"));
    assert!(csv_loaded.content.contains("bob,25"));
}

#[tokio::test]
async fn test_invalid_utf8_text_regression_guard() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.txt");

    let mut bytes = vec![b'a'; 600];
    bytes.push(0xFF);
    bytes.extend_from_slice(&[b'\n', b'b', b'\n']);
    std::fs::write(&path, bytes).unwrap();

    let err = load_context(&path).await.unwrap_err().to_string();
    assert!(err.contains("not valid UTF-8 text"));
}
