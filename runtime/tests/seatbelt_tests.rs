use openskills_runtime::{OpenSkillRuntime, ExecutionOptions, RuntimeExecutionStatus};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[test]
#[cfg(target_os = "macos")]
fn test_native_seatbelt_execution_shell() {
    // Create a temporary directory for our test skill
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("native-test-skill");
    fs::create_dir(&skill_dir).unwrap();

    // Create SKILL.md
    let manifest = r#"---
name: native-test-skill
description: A native test skill using shell script
user_invocable: true
allowed_tools: []
---
"#;
    fs::write(skill_dir.join("SKILL.md"), manifest).unwrap();

    // Create script.sh
    // This script prints the input JSON to stdout
    let script_content = r#"#!/bin/bash
echo '{"status": "success", "message": "hello from shell"}'
"#;
    let script_path = skill_dir.join("script.sh");
    fs::write(&script_path, script_content).unwrap();
    
    // Make it executable
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    // Initialize runtime with the custom directory
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Execute the skill
    let options = ExecutionOptions {
        timeout_ms: Some(5000),
        input: Some(json!({"foo": "bar"})),
        ..Default::default()
    };

    let result = runtime.execute_skill("native-test-skill", options);
    
    match result {
        Ok(exec_result) => {
            println!("Stdout: {}", exec_result.stdout);
            println!("Stderr: {}", exec_result.stderr);
            assert!(matches!(exec_result.audit.exit_status, RuntimeExecutionStatus::Success), "Execution failed: {:?}", exec_result.audit.exit_status);
            println!("Output: {}", exec_result.output);
            
            // Check output content
            assert_eq!(exec_result.output["message"], "hello from shell");
        },
        Err(e) => {
            panic!("Native execution failed: {:?}", e);
        }
    }
}

#[test]
#[cfg(target_os = "macos")]
fn test_native_seatbelt_file_access_denied() {
    // Test that the sandbox actually restricts access
    
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("security-test-skill");
    fs::create_dir(&skill_dir).unwrap();
    
    // Create a secret file outside the skill directory
    let secret_path = temp_dir.path().join("secret.txt");
    fs::write(&secret_path, "secret data").unwrap();

    let manifest = r#"---
name: security-test-skill
description: Try to read unauthorized file
---
"#;
    fs::write(skill_dir.join("SKILL.md"), manifest).unwrap();

    let script_content = format!(r#"#!/bin/bash
cat {}
"#, secret_path.display());
    
    let script_path = skill_dir.join("script.sh");
    fs::write(&script_path, script_content).unwrap();
    fs::set_permissions(&script_path, fs::metadata(&script_path).unwrap().permissions()).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();
    
    let result = runtime.execute_skill("security-test-skill", Default::default());
    
    match result {
        Ok(exec_result) => {
            // If it succeeds, check that output doesn't contain secret
            // cat will fail, so stderr should have permission denied
            println!("Stdout: {}", exec_result.stdout);
            println!("Stderr: {}", exec_result.stderr);
            
            assert!(!exec_result.stdout.contains("secret data"), "Sandbox failed! Read secret data");
            // Either stderr has "Permission denied" or "Operation not permitted"
            // or the process failed.
        },
        Err(e) => {
            // Execution failure is also acceptable
            println!("Execution failed as expected: {:?}", e);
        }
    }
}
