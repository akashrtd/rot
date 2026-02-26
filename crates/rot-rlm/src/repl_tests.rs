#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_repl_env_basic() {
        let mut repl = ReplEnv::new();
        
        let mut context_file = NamedTempFile::new().unwrap();
        writeln!(context_file, "Hello RLM Context").unwrap();
        
        repl.init(context_file.path().to_str().unwrap()).await.unwrap();
        
        // Execute simple echo
        let res = repl.execute("echo 'test output'").await.unwrap();
        assert_eq!(res.stdout.trim(), "test output");
        assert_eq!(res.exit_code, Some(0));
        
        // Execute multiline
        let res2 = repl.execute("FOO='bar'\necho $FOO").await.unwrap();
        assert_eq!(res2.stdout.trim(), "bar");
        assert_eq!(res2.exit_code, Some(0));
        
        // Persistence test (the shell remains the same)
        let res3 = repl.execute("echo $FOO").await.unwrap();
        assert_eq!(res3.stdout.trim(), "bar");
    }

    #[tokio::test]
    async fn test_repl_env_helpers() {
        let mut repl = ReplEnv::new();
        
        let mut context_file = NamedTempFile::new().unwrap();
        let content = "Context ABC 123";
        writeln!(context_file, "{}", content).unwrap();
        
        repl.init(context_file.path().to_str().unwrap()).await.unwrap();
        
        // Test get_context()
        let res = repl.execute("get_context").await.unwrap();
        assert_eq!(res.stdout.trim(), content);
        
        // Test FINAL()
        let res_final = repl.execute("FINAL 'my answer'").await.unwrap();
        assert_eq!(res_final.stdout.trim(), "FINAL_ANSWER:my answer");
        
        // Test SHOW_VARS()
        repl.execute("export TEST_VAR=123").await.unwrap();
        let res_vars = repl.execute("SHOW_VARS").await.unwrap();
        println!("VARS OUTPUT: {}", res_vars.stdout);
        assert!(res_vars.stdout.contains("TEST_VAR"));
    }
}
