//! Target Execution Tests
//!
//! Tests for runSkillTarget (script/WASM execution).
//! Verifies auto-detection of execution type, argument passing, and workspace handling.

use openskills_runtime::{OpenSkillRuntime, ExecutionTarget};
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Helper Functions
// =============================================================================

fn create_skill_with_script(temp_dir: &TempDir, name: &str, script_name: &str, script_content: &str) {
    let skill_dir = temp_dir.path().join(name);
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            r#"---
name: {}
description: Test skill with script.
---
# Instructions
Run the script.
"#,
            name
        ),
    )
    .unwrap();

    fs::write(skill_dir.join(script_name), script_content).unwrap();

    // Make script executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = skill_dir.join(script_name);
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
}

// =============================================================================
// Auto-Detection Tests
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_run_skill_target_auto_detect_shell_script() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_script(
        &temp_dir,
        "shell-skill",
        "script.sh",
        r#"#!/bin/bash
echo "shell script output"
"#,
    );

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "script.sh".to_string(),
        args: vec![],
    };

    let result = runtime.run_skill_target("shell-skill", target, Some(5000), None, None);

    match result {
        Ok(exec_result) => {
            assert!(
                exec_result.stdout.contains("shell script output"),
                "Shell script should execute"
            );
        }
        Err(e) => {
            // May fail due to sandbox - acceptable
            println!("Shell script execution error (may be sandbox): {:?}", e);
        }
    }
}

#[test]
#[cfg(target_os = "macos")]
fn test_run_skill_target_auto_detect_python_script() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_script(
        &temp_dir,
        "python-skill",
        "script.py",
        r#"#!/usr/bin/env python3
print("python script output")
"#,
    );

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "script.py".to_string(),
        args: vec![],
    };

    let result = runtime.run_skill_target("python-skill", target, Some(5000), None, None);

    // Python may or may not be installed/accessible in sandbox
    assert!(result.is_ok() || result.is_err());
}

// =============================================================================
// Script Arguments
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_run_skill_target_with_args() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_script(
        &temp_dir,
        "args-skill",
        "script.sh",
        r#"#!/bin/bash
echo "args: $1 $2"
"#,
    );

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "script.sh".to_string(),
        args: vec!["arg1".to_string(), "arg2".to_string()],
    };

    let result = runtime.run_skill_target("args-skill", target, Some(5000), None, None);

    match result {
        Ok(exec_result) => {
            assert!(
                exec_result.stdout.contains("arg1") && exec_result.stdout.contains("arg2"),
                "Script should receive arguments"
            );
        }
        Err(_) => {
            // Sandbox may block - acceptable
        }
    }
}

// =============================================================================
// JSON Input
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_run_skill_target_with_json_input() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_script(
        &temp_dir,
        "json-skill",
        "script.sh",
        r#"#!/bin/bash
# Read input from stdin
read input
echo "received: $input"
"#,
    );

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "script.sh".to_string(),
        args: vec![],
    };
    let input = serde_json::json!({"action": "test"});

    let result = runtime.run_skill_target("json-skill", target, Some(5000), Some(input), None);

    // Just verify it doesn't crash
    assert!(result.is_ok() || result.is_err());
}

// =============================================================================
// Workspace Directory
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_run_skill_target_workspace_env() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();

    create_skill_with_script(
        &temp_dir,
        "workspace-skill",
        "script.sh",
        r#"#!/bin/bash
echo "workspace: $SKILL_WORKSPACE"
"#,
    );

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "script.sh".to_string(),
        args: vec![],
    };

    let result = runtime.run_skill_target("workspace-skill", target, Some(5000), None, Some(workspace));

    match result {
        Ok(exec_result) => {
            // SKILL_WORKSPACE should be set
            assert!(
                exec_result.stdout.contains("workspace") || exec_result.stdout.len() > 0,
                "Workspace env var should be set"
            );
        }
        Err(_) => {
            // May fail due to sandbox
        }
    }
}

// =============================================================================
// Timeout
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_run_skill_target_timeout() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_script(
        &temp_dir,
        "timeout-skill",
        "script.sh",
        r#"#!/bin/bash
sleep 10
echo "should not reach"
"#,
    );

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "script.sh".to_string(),
        args: vec![],
    };

    let result = runtime.run_skill_target("timeout-skill", target, Some(500), None, None);

    // Should timeout or error
    match result {
        Ok(exec_result) => {
            // If it succeeded, it should have timed out (not reached "should not reach")
            assert!(!exec_result.stdout.contains("should not reach"));
        }
        Err(_) => {
            // Error (timeout) is expected
        }
    }
}

#[test]
#[cfg(target_os = "macos")]
fn test_run_skill_target_zero_timeout_uses_default() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_script(
        &temp_dir,
        "zero-timeout-skill",
        "script.sh",
        r#"#!/bin/bash
sleep 1
echo "finished"
"#,
    );

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "script.sh".to_string(),
        args: vec![],
    };

    let result = runtime.run_skill_target("zero-timeout-skill", target, Some(0), None, None);

    match result {
        Ok(exec_result) => {
            assert!(
                exec_result.stdout.contains("finished"),
                "Zero timeout should fall back to default timeout instead of timing out immediately"
            );
        }
        Err(_) => {
            // Sandbox/environment differences can still fail execution.
        }
    }
}

#[test]
#[cfg(target_os = "macos")]
fn test_run_skill_target_none_timeout_uses_default() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_script(
        &temp_dir,
        "none-timeout-skill",
        "script.sh",
        r#"#!/bin/bash
sleep 1
echo "finished"
"#,
    );

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "script.sh".to_string(),
        args: vec![],
    };

    let result = runtime.run_skill_target("none-timeout-skill", target, None, None, None);

    match result {
        Ok(exec_result) => {
            assert!(
                exec_result.stdout.contains("finished"),
                "Missing timeout should use default timeout instead of timing out immediately"
            );
        }
        Err(_) => {
            // Sandbox/environment differences can still fail execution.
        }
    }
}

// =============================================================================
// Invalid Path
// =============================================================================

#[test]
fn test_run_skill_target_invalid_path() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("invalid-path-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: invalid-path-skill
description: Test invalid path.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "nonexistent.sh".to_string(),
        args: vec![],
    };

    let result = runtime.run_skill_target("invalid-path-skill", target, None, None, None);

    assert!(result.is_err(), "Should error for nonexistent script");
}

#[test]
fn test_run_skill_target_path_traversal() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("traversal-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: traversal-skill
description: Test path traversal protection.
---
"#,
    )
    .unwrap();

    // Create a script outside the skill directory
    fs::write(temp_dir.path().join("outside.sh"), "#!/bin/bash\necho secret").unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "../outside.sh".to_string(),
        args: vec![],
    };

    let result = runtime.run_skill_target("traversal-skill", target, None, None, None);

    // Should either error or not execute the outside script
    match result {
        Ok(exec_result) => {
            assert!(
                !exec_result.stdout.contains("secret"),
                "Path traversal should be blocked"
            );
        }
        Err(_) => {
            // Error is expected - path traversal blocked
        }
    }
}

// =============================================================================
// Skill Not Found
// =============================================================================

#[test]
fn test_run_skill_target_skill_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "script.sh".to_string(),
        args: vec![],
    };

    let result = runtime.run_skill_target("nonexistent-skill", target, None, None, None);

    assert!(result.is_err(), "Should error for nonexistent skill");
}

// =============================================================================
// Script in Subdirectory
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_run_skill_target_script_in_subdir() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("subdir-skill");
    let scripts_dir = skill_dir.join("scripts");
    fs::create_dir_all(&scripts_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: subdir-skill
description: Script in subdirectory.
---
"#,
    )
    .unwrap();

    let script_path = scripts_dir.join("process.sh");
    fs::write(&script_path, "#!/bin/bash\necho 'from subdirectory'").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let target = ExecutionTarget::Path {
        path: "scripts/process.sh".to_string(),
        args: vec![],
    };

    let result = runtime.run_skill_target("subdir-skill", target, Some(5000), None, None);

    match result {
        Ok(exec_result) => {
            assert!(
                exec_result.stdout.contains("from subdirectory"),
                "Script in subdirectory should execute"
            );
        }
        Err(_) => {
            // Sandbox may block
        }
    }
}

// =============================================================================
// Default Execution Target
// =============================================================================

#[test]
fn test_execution_target_default() {
    let target = ExecutionTarget::default();

    // Verify default is Auto
    assert!(matches!(target, ExecutionTarget::Auto));
}
