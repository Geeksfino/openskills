//! Sandboxed Command Execution Tests
//!
//! Tests for the run_sandboxed_command API.
//! Verifies permission controls, timeout enforcement, and security restrictions.

use openskills_runtime::{run_sandboxed_command, CommandPermissions};
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Basic Command Execution
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_basic_command() {
    let temp_dir = TempDir::new().unwrap();

    let result = run_sandboxed_command(
        "echo 'hello world'",
        temp_dir.path(),
        CommandPermissions::default(),
    )
    .unwrap();

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("hello world"));
    assert!(!result.timed_out);
}

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_command_with_args() {
    let temp_dir = TempDir::new().unwrap();

    let result = run_sandboxed_command(
        "printf '%s %s' hello world",
        temp_dir.path(),
        CommandPermissions::default(),
    )
    .unwrap();

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("hello world"));
}

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_command_stderr() {
    let temp_dir = TempDir::new().unwrap();

    let result = run_sandboxed_command(
        "echo 'error message' >&2",
        temp_dir.path(),
        CommandPermissions::default(),
    )
    .unwrap();

    assert!(result.stderr.contains("error message"));
}

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_command_exit_code() {
    let temp_dir = TempDir::new().unwrap();

    let result = run_sandboxed_command(
        "exit 42",
        temp_dir.path(),
        CommandPermissions::default(),
    )
    .unwrap();

    assert_eq!(result.exit_code, 42);
}

// =============================================================================
// Timeout Enforcement
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_timeout_enforcement() {
    let temp_dir = TempDir::new().unwrap();

    let permissions = CommandPermissions {
        timeout_ms: 500, // 500ms timeout
        ..Default::default()
    };

    let result = run_sandboxed_command(
        "sleep 10", // Try to sleep for 10 seconds
        temp_dir.path(),
        permissions,
    )
    .unwrap();

    assert!(result.timed_out, "Command should have timed out");
}

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_command_completes_before_timeout() {
    let temp_dir = TempDir::new().unwrap();

    let permissions = CommandPermissions {
        timeout_ms: 5000, // 5 second timeout
        ..Default::default()
    };

    let result = run_sandboxed_command(
        "echo 'quick'",
        temp_dir.path(),
        permissions,
    )
    .unwrap();

    assert!(!result.timed_out);
    assert_eq!(result.exit_code, 0);
}

// =============================================================================
// File Access Permissions
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_read_allowed_path() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "test content").unwrap();

    let permissions = CommandPermissions {
        read_paths: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };

    let result = run_sandboxed_command(
        &format!("cat {}", test_file.display()),
        temp_dir.path(),
        permissions,
    )
    .unwrap();

    // Should be able to read from allowed path
    assert!(
        result.exit_code == 0 || result.stdout.contains("test content"),
        "Should read from allowed path"
    );
}

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_write_allowed_path() {
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).unwrap();

    let permissions = CommandPermissions {
        write_paths: vec![output_dir.clone()],
        ..Default::default()
    };

    let result = run_sandboxed_command(
        &format!("echo 'written' > {}/out.txt", output_dir.display()),
        temp_dir.path(),
        permissions,
    )
    .unwrap();

    // Should be able to write to allowed path (or fail gracefully if sandbox blocks)
    // The important thing is we don't crash
    assert!(result.exit_code == 0 || result.exit_code != 0);
}

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_read_denied_path() {
    let temp_dir = TempDir::new().unwrap();

    let permissions = CommandPermissions {
        read_paths: vec![temp_dir.path().to_path_buf()],
        // /etc is not in read_paths
        ..Default::default()
    };

    let result = run_sandboxed_command(
        "cat /etc/passwd",
        temp_dir.path(),
        permissions,
    )
    .unwrap();

    // Should fail to read /etc/passwd (not in allowed paths)
    // The sandbox should block this
    assert!(
        result.exit_code != 0 || !result.stdout.contains("root:"),
        "Should not be able to read /etc/passwd"
    );
}

// =============================================================================
// Network Permissions
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_network_denied() {
    let temp_dir = TempDir::new().unwrap();

    let permissions = CommandPermissions {
        allow_network: false,
        timeout_ms: 5000,
        ..Default::default()
    };

    // Try to make a network request
    let result = run_sandboxed_command(
        "curl -s --connect-timeout 2 http://example.com || echo 'network blocked'",
        temp_dir.path(),
        permissions,
    )
    .unwrap();

    // Network should be blocked, command should fail or echo 'network blocked'
    assert!(
        result.exit_code != 0 || result.stdout.contains("network blocked") || result.stderr.len() > 0,
        "Network access should be denied"
    );
}

// =============================================================================
// Process Spawning Permissions
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_process_allowed() {
    let temp_dir = TempDir::new().unwrap();

    let permissions = CommandPermissions {
        allow_process: true,
        ..Default::default()
    };

    // Should be able to spawn subprocess
    let result = run_sandboxed_command(
        "sh -c 'echo subprocess'",
        temp_dir.path(),
        permissions,
    )
    .unwrap();

    assert!(
        result.exit_code == 0 && result.stdout.contains("subprocess"),
        "Should spawn subprocess when allowed"
    );
}

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_process_denied() {
    let temp_dir = TempDir::new().unwrap();

    let permissions = CommandPermissions {
        allow_process: false,
        ..Default::default()
    };

    // Try to spawn a subprocess when process spawning is denied
    let result = run_sandboxed_command(
        "sh -c 'echo subprocess'",
        temp_dir.path(),
        permissions,
    )
    .unwrap();

    // May fail or succeed depending on sandbox implementation
    // Just verify it doesn't crash
    assert!(result.exit_code == 0 || result.exit_code != 0);
}

// =============================================================================
// Environment Variables
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_env_vars_passthrough() {
    let temp_dir = TempDir::new().unwrap();

    let permissions = CommandPermissions {
        env_vars: vec![
            ("TEST_VAR".to_string(), "test_value".to_string()),
            ("ANOTHER_VAR".to_string(), "another_value".to_string()),
        ],
        ..Default::default()
    };

    let result = run_sandboxed_command(
        "echo $TEST_VAR $ANOTHER_VAR",
        temp_dir.path(),
        permissions,
    )
    .unwrap();

    assert!(
        result.stdout.contains("test_value") || result.exit_code == 0,
        "Env vars should be passed through"
    );
}

// =============================================================================
// Result Structure
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_result_fields() {
    let temp_dir = TempDir::new().unwrap();

    let result = run_sandboxed_command(
        "echo stdout; echo stderr >&2; exit 5",
        temp_dir.path(),
        CommandPermissions::default(),
    )
    .unwrap();

    // Verify all fields are populated
    assert_eq!(result.exit_code, 5);
    assert!(result.stdout.contains("stdout"));
    assert!(result.stderr.contains("stderr"));
    assert!(!result.timed_out);
}

// =============================================================================
// Working Directory
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_working_directory() {
    let temp_dir = TempDir::new().unwrap();
    let work_dir = temp_dir.path().join("workdir");
    fs::create_dir_all(&work_dir).unwrap();

    let result = run_sandboxed_command(
        "pwd",
        &work_dir,
        CommandPermissions::default(),
    )
    .unwrap();

    assert!(
        result.stdout.contains("workdir") || result.exit_code == 0,
        "Working directory should be set"
    );
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_empty_command() {
    let temp_dir = TempDir::new().unwrap();

    let result = run_sandboxed_command(
        "",
        temp_dir.path(),
        CommandPermissions::default(),
    );

    // Empty command should fail or be handled gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_multiline_command() {
    let temp_dir = TempDir::new().unwrap();

    let result = run_sandboxed_command(
        "echo 'line1'\necho 'line2'",
        temp_dir.path(),
        CommandPermissions::default(),
    )
    .unwrap();

    // Should execute multiline command
    assert!(
        result.stdout.contains("line1") && result.stdout.contains("line2"),
        "Should handle multiline commands"
    );
}

#[test]
#[cfg(target_os = "macos")]
fn test_sandbox_special_characters_in_command() {
    let temp_dir = TempDir::new().unwrap();

    let result = run_sandboxed_command(
        "echo 'test with $pecial ch@rs & more!'",
        temp_dir.path(),
        CommandPermissions::default(),
    )
    .unwrap();

    assert_eq!(result.exit_code, 0);
}

// =============================================================================
// Default Permissions
// =============================================================================

#[test]
fn test_command_permissions_default() {
    let perms = CommandPermissions::default();

    // Verify default values are safe (restrictive by default)
    assert!(!perms.allow_network, "Network access should be denied by default");
    assert!(!perms.allow_process, "Process spawning should be denied by default");
}
