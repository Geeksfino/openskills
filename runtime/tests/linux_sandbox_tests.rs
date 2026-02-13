//! Linux Landlock sandbox tests.
//!
//! These tests verify the Linux sandbox implementation using Landlock.
//! They are only compiled and run on Linux systems.

#[cfg(target_os = "linux")]
use openskills_runtime::{OpenSkillRuntime, ExecutionOptions, RuntimeExecutionStatus};
#[cfg(target_os = "linux")]
use serde_json::json;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use tempfile::TempDir;

/// Check if Landlock is supported on the current system.
#[cfg(target_os = "linux")]
fn is_landlock_supported() -> bool {
    // Check kernel version (Landlock requires 5.13+)
    if let Ok(version) = std::fs::read_to_string("/proc/version") {
        // Extract kernel version like "5.15.0" from the version string
        if let Some(start) = version.find("Linux version ") {
            let version_part = &version[start + 14..];
            if let Some(end) = version_part.find(|c: char| !c.is_ascii_digit() && c != '.') {
                let version_str = &version_part[..end];
                let parts: Vec<u32> = version_str
                    .split('.')
                    .filter_map(|p| p.parse().ok())
                    .collect();
                if parts.len() >= 2 {
                    let major = parts[0];
                    let minor = parts[1];
                    if major < 5 || (major == 5 && minor < 13) {
                        return false;
                    }
                }
            }
        }
    }
    
    // Check if Landlock is enabled in LSM
    if let Ok(lsm) = std::fs::read_to_string("/sys/kernel/security/lsm") {
        return lsm.contains("landlock");
    }
    
    false
}

#[test]
#[cfg(target_os = "linux")]
fn test_native_linux_execution_shell() {
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
    let script_content = r#"#!/bin/bash
echo '{"status": "success", "message": "hello from linux shell"}'
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
        timeout_ms: Some(10000),
        input: Some(json!({"foo": "bar"})),
        ..Default::default()
    };

    let result = runtime.execute_skill("native-test-skill", options);
    
    match result {
        Ok(exec_result) => {
            println!("Stdout: {}", exec_result.stdout);
            println!("Stderr: {}", exec_result.stderr);
            assert!(
                matches!(exec_result.audit.exit_status, RuntimeExecutionStatus::Success),
                "Execution failed: {:?}",
                exec_result.audit.exit_status
            );
            println!("Output: {}", exec_result.output);
            
            // Check output content
            assert_eq!(exec_result.output["message"], "hello from linux shell");
        }
        Err(e) => {
            panic!("Native execution failed: {:?}", e);
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_native_linux_execution_python() {
    // Check if python3 is available
    if !std::path::Path::new("/usr/bin/python3").exists() {
        println!("Skipping test: python3 not found");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("python-test-skill");
    fs::create_dir(&skill_dir).unwrap();

    let manifest = r#"---
name: python-test-skill
description: A native test skill using Python
user_invocable: true
allowed_tools: []
---
"#;
    fs::write(skill_dir.join("SKILL.md"), manifest).unwrap();

    let script_content = r#"#!/usr/bin/env python3
import json
import sys

# Read input from stdin
input_data = sys.stdin.read()
try:
    data = json.loads(input_data)
except:
    data = {}

# Output JSON
print(json.dumps({
    "status": "success",
    "message": "hello from python",
    "received": data.get("test_value", "none")
}))
"#;
    let script_path = skill_dir.join("script.py");
    fs::write(&script_path, script_content).unwrap();
    
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let options = ExecutionOptions {
        timeout_ms: Some(10000),
        input: Some(json!({"test_value": "hello_world"})),
        ..Default::default()
    };

    let result = runtime.execute_skill("python-test-skill", options);
    
    match result {
        Ok(exec_result) => {
            println!("Stdout: {}", exec_result.stdout);
            println!("Stderr: {}", exec_result.stderr);
            assert!(
                matches!(exec_result.audit.exit_status, RuntimeExecutionStatus::Success),
                "Execution failed: {:?}",
                exec_result.audit.exit_status
            );
            
            assert_eq!(exec_result.output["message"], "hello from python");
            assert_eq!(exec_result.output["received"], "hello_world");
        }
        Err(e) => {
            panic!("Python execution failed: {:?}", e);
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_linux_sandbox_blocks_sensitive_paths() {
    if !is_landlock_supported() {
        println!("Skipping test: Landlock not supported on this kernel");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("security-test-skill");
    fs::create_dir(&skill_dir).unwrap();

    // Create a fake sensitive file to test access denial
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let sensitive_dir = format!("{}/.ssh", home);
    
    // Only test if .ssh directory exists
    if !std::path::Path::new(&sensitive_dir).exists() {
        println!("Skipping test: ~/.ssh does not exist");
        return;
    }

    let manifest = r#"---
name: security-test-skill
description: Try to read sensitive paths
user_invocable: true
allowed_tools: []
---
"#;
    fs::write(skill_dir.join("SKILL.md"), manifest).unwrap();

    // Script attempts to list ~/.ssh contents
    let script_content = format!(r#"#!/bin/bash
if ls -la {} 2>/dev/null; then
    echo '{{"status": "error", "message": "sandbox failed - could read sensitive directory"}}'
else
    echo '{{"status": "success", "message": "sandbox blocked access as expected"}}'
fi
"#, sensitive_dir);
    
    let script_path = skill_dir.join("script.sh");
    fs::write(&script_path, script_content).unwrap();
    
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.execute_skill("security-test-skill", Default::default());
    
    match result {
        Ok(exec_result) => {
            println!("Stdout: {}", exec_result.stdout);
            println!("Stderr: {}", exec_result.stderr);
            
            // With Landlock, the script should report that access was blocked
            // OR the script itself might fail
            if exec_result.output["message"] == "sandbox failed - could read sensitive directory" {
                panic!("Sandbox failed! Script could read sensitive ~/.ssh directory");
            }
        }
        Err(e) => {
            // Sandbox rejection is also acceptable
            println!("Execution failed as expected: {:?}", e);
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_linux_sandbox_allows_workspace_writes() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("write-test-skill");
    let workspace_dir = temp_dir.path().join("workspace");
    fs::create_dir(&skill_dir).unwrap();
    fs::create_dir(&workspace_dir).unwrap();

    let manifest = r#"---
name: write-test-skill
description: Test writing to workspace directory
user_invocable: true
allowed_tools: ["Write"]
---
"#;
    fs::write(skill_dir.join("SKILL.md"), manifest).unwrap();

    let script_content = r#"#!/bin/bash
# Write a test file to the workspace
WORKSPACE="${SKILL_WORKSPACE:-$(pwd)}"
echo "test content" > "$WORKSPACE/output.txt"

if [ -f "$WORKSPACE/output.txt" ]; then
    echo '{"status": "success", "message": "file written successfully"}'
else
    echo '{"status": "error", "message": "failed to write file"}'
fi
"#;
    let script_path = skill_dir.join("script.sh");
    fs::write(&script_path, script_content).unwrap();
    
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path())
        .with_workspace_dir(&workspace_dir);
    runtime.discover_skills().unwrap();

    let options = ExecutionOptions {
        timeout_ms: Some(10000),
        ..Default::default()
    };

    let result = runtime.execute_skill("write-test-skill", options);
    
    match result {
        Ok(exec_result) => {
            println!("Stdout: {}", exec_result.stdout);
            println!("Stderr: {}", exec_result.stderr);
            assert!(
                matches!(exec_result.audit.exit_status, RuntimeExecutionStatus::Success),
                "Execution failed: {:?}",
                exec_result.audit.exit_status
            );
            
            assert_eq!(exec_result.output["message"], "file written successfully");
            
            // Verify the file was actually created
            let output_file = workspace_dir.join("output.txt");
            assert!(output_file.exists(), "Output file was not created");
            let content = fs::read_to_string(&output_file).unwrap();
            assert_eq!(content.trim(), "test content");
        }
        Err(e) => {
            panic!("Execution failed: {:?}", e);
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_linux_sandbox_timeout() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("timeout-test-skill");
    fs::create_dir(&skill_dir).unwrap();

    let manifest = r#"---
name: timeout-test-skill
description: Test timeout handling
user_invocable: true
allowed_tools: []
---
"#;
    fs::write(skill_dir.join("SKILL.md"), manifest).unwrap();

    // Script that sleeps longer than the timeout
    let script_content = r#"#!/bin/bash
sleep 60
echo '{"status": "success"}'
"#;
    let script_path = skill_dir.join("script.sh");
    fs::write(&script_path, script_content).unwrap();
    
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let options = ExecutionOptions {
        timeout_ms: Some(1000), // 1 second timeout
        ..Default::default()
    };

    let result = runtime.execute_skill("timeout-test-skill", options);
    
    match result {
        Ok(exec_result) => {
            // Should report timeout
            assert!(
                matches!(exec_result.audit.exit_status, RuntimeExecutionStatus::Timeout),
                "Expected timeout, got: {:?}",
                exec_result.audit.exit_status
            );
        }
        Err(e) => {
            // Timeout error is also acceptable
            println!("Got error as expected: {:?}", e);
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_run_sandboxed_command() {
    use openskills_runtime::{run_sandboxed_command, CommandPermissions};

    let temp_dir = TempDir::new().unwrap();
    let working_dir = temp_dir.path();

    let permissions = CommandPermissions {
        allow_network: false,
        allow_process: true,
        read_paths: vec![working_dir.to_path_buf()],
        write_paths: vec![working_dir.to_path_buf()],
        env_vars: vec![],
        timeout_ms: 10000,
    };

    let result = run_sandboxed_command("echo 'hello from sandbox'", working_dir, permissions);
    
    match result {
        Ok(cmd_result) => {
            assert_eq!(cmd_result.exit_code, 0);
            assert!(cmd_result.stdout.contains("hello from sandbox"));
            assert!(!cmd_result.timed_out);
        }
        Err(e) => {
            // On older kernels without Landlock, we still expect basic execution to work
            // with NO_NEW_PRIVS fallback
            panic!("Sandboxed command failed: {:?}", e);
        }
    }
}
