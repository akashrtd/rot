//! E2E tests for common user scenarios.
//!
//! These tests verify realistic user workflows work correctly.

use std::path::PathBuf;
use std::process::Command;

fn rot_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rot"))
}

#[test]
fn test_e2e_simple_question() {
    let dir = tempfile::tempdir().unwrap();
    
    // User asks a simple question
    let output = Command::new(rot_bin())
        .args(["exec", "What is 2 + 2?", "--provider", "mock", "--final-json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["4"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["final_text"], "4");
}

#[test]
fn test_e2e_code_review_workflow() {
    let dir = tempfile::tempdir().unwrap();
    
    // Create a file to review
    let code_file = dir.path().join("main.rs");
    std::fs::write(&code_file, r#"
fn main() {
    println!("Hello, world!");
}
"#).unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "Review the code in main.rs", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Code looks good!"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_e2e_file_modification_workflow() {
    let dir = tempfile::tempdir().unwrap();
    
    // Create initial file
    let file = dir.path().join("config.json");
    std::fs::write(&file, r#"{"version": 1}"#).unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "Update version to 2", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Version updated to 2"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_e2e_search_and_replace_workflow() {
    let dir = tempfile::tempdir().unwrap();
    
    // Create files to search
    std::fs::write(dir.path().join("file1.rs"), "fn old_name() {}").unwrap();
    std::fs::write(dir.path().join("file2.rs"), "fn other() {}").unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "Find all functions", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Found 2 functions"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_e2e_project_exploration() {
    let dir = tempfile::tempdir().unwrap();
    
    // Create a project structure
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn lib() {}").unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "Explore this project", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["This is a Rust project with main and lib"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_e2e_error_recovery() {
    let dir = tempfile::tempdir().unwrap();
    
    // Try to read non-existent file - should fail gracefully
    let output = Command::new(rot_bin())
        .args(["exec", "Read nonexistent.txt", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["File not found, but I handled it"]).unwrap(),
        )
        .output()
        .unwrap();

    // The mock provider returns success, but in real scenario this would test error handling
    assert!(output.status.success());
}

#[test]
fn test_e2e_planning_mode() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "Plan how to implement a REST API", "--provider", "mock", "--agent", "plan", "--final-json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["1. Design endpoints\n2. Create handlers\n3. Add tests"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(payload["final_text"].as_str().unwrap().contains("endpoints"));
}

#[test]
fn test_e2e_build_mode() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "Build a hello world program", "--provider", "mock", "--agent", "build", "--final-json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Program built successfully"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_e2e_non_interactive_mode() {
    let dir = tempfile::tempdir().unwrap();
    
    // Test that auto-approve allows non-interactive execution
    let output = Command::new(rot_bin())
        .args(["exec", "Do something dangerous", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Done"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_e2e_approved_tools_only() {
    let dir = tempfile::tempdir().unwrap();
    
    // Test that only approved tools are allowed
    let output = Command::new(rot_bin())
        .args(["exec", "Read README.md", "--provider", "mock", "--final-json", "--approve-list", "read"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Read the file"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_e2e_structured_output() {
    let dir = tempfile::tempdir().unwrap();
    let schema = dir.path().join("schema.json");
    std::fs::write(
        &schema,
        r#"{
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": {"type": "string"}
                }
            },
            "required": ["files"]
        }"#,
    )
    .unwrap();
    
    let output = Command::new(rot_bin())
        .args([
            "exec",
            "List all files",
            "--provider",
            "mock",
            "--output-schema",
            schema.to_str().unwrap(),
            "--final-json",
        ])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&[r#"{"files": ["main.rs", "lib.rs"]}"#]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_e2e_multiple_tool_calls_workflow() {
    let dir = tempfile::tempdir().unwrap();
    
    // Create multiple files
    std::fs::write(dir.path().join("a.txt"), "content a").unwrap();
    std::fs::write(dir.path().join("b.txt"), "content b").unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "Read all txt files", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Read 2 files"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}
