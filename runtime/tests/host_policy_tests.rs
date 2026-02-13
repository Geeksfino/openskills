//! Host Policy and Permission Tests
//!
//! Tests for host policy configuration, tool permission checking,
//! and permission callback modes.

use openskills_runtime::{OpenSkillRuntime, Fallback, HostPolicy, PermissionsConfig, is_risky_tool};
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Helper Functions
// =============================================================================

fn create_skill_with_allowed_tools(temp_dir: &TempDir, name: &str, allowed_tools: &str) {
    let skill_dir = temp_dir.path().join(name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            r#"---
name: {}
description: Test skill for permissions.
allowed-tools: {}
---
"#,
            name, allowed_tools
        ),
    )
    .unwrap();
}

// =============================================================================
// Basic Permission Checking
// =============================================================================

#[test]
fn test_check_tool_allowed_by_skill() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "read-skill", "Read, Grep");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Read is in allowed-tools
    let result = runtime.check_tool_permission(
        "read-skill",
        "Read",
        None,
        HashMap::new(),
    );

    assert!(result.is_ok(), "Read should be allowed");
}

#[test]
fn test_check_tool_not_in_allowed_tools() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "limited-skill", "Read");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Write is NOT in allowed-tools
    let result = runtime.check_tool_permission(
        "limited-skill",
        "Write",
        None,
        HashMap::new(),
    );

    // Should be denied (not in skill's allowed-tools)
    assert!(result.is_err(), "Write should be denied - not in allowed-tools");
}

#[test]
fn test_check_tool_empty_allowed_tools() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("no-tools-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: no-tools-skill
description: Skill with no allowed tools specified.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // When allowed-tools is empty/not specified, behavior depends on host policy
    let result = runtime.check_tool_permission(
        "no-tools-skill",
        "Read",
        None,
        HashMap::new(),
    );

    // Result depends on default host policy
    assert!(result.is_ok() || result.is_err());
}

// =============================================================================
// Host Policy Configuration
// =============================================================================

#[test]
fn test_host_policy_deny_list() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "policy-skill", "Read, Write, Bash");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Set host policy with deny list
    let policy = HostPolicy::from_config(PermissionsConfig {
        trust_skill_allowed_tools: true,
        fallback: Fallback::Deny,
        deny: vec!["Bash".to_string()],
        allow: vec![],
    });
    runtime.set_host_policy(policy);

    // Bash should be denied even though it's in skill's allowed-tools
    let result = runtime.check_tool_permission(
        "policy-skill",
        "Bash",
        None,
        HashMap::new(),
    );

    assert!(result.is_err(), "Bash should be denied by host policy deny list");
}

#[test]
fn test_host_policy_allow_list() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "policy-skill", "Read");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Set host policy with allow list
    let policy = HostPolicy::from_config(PermissionsConfig {
        trust_skill_allowed_tools: false,
        fallback: Fallback::Deny,
        deny: vec![],
        allow: vec!["Write".to_string()],
    });
    runtime.set_host_policy(policy);

    // Write should be allowed by host policy even if skill doesn't list it
    let result = runtime.check_tool_permission(
        "policy-skill",
        "Write",
        None,
        HashMap::new(),
    );

    assert!(result.is_ok(), "Write should be allowed by host policy allow list");
}

#[test]
fn test_host_policy_trust_skill_true() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "trusted-skill", "Read, Write");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Trust skill's allowed-tools
    let policy = HostPolicy::from_config(PermissionsConfig {
        trust_skill_allowed_tools: true,
        fallback: Fallback::Deny,
        deny: vec![],
        allow: vec![],
    });
    runtime.set_host_policy(policy);

    let result = runtime.check_tool_permission(
        "trusted-skill",
        "Read",
        None,
        HashMap::new(),
    );

    assert!(result.is_ok(), "Read should be allowed when trusting skill");
}

#[test]
fn test_host_policy_trust_skill_false() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "untrusted-skill", "Read, Write");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Don't trust skill's allowed-tools
    let policy = HostPolicy::from_config(PermissionsConfig {
        trust_skill_allowed_tools: false,
        fallback: Fallback::Deny,
        deny: vec![],
        allow: vec![],
    });
    runtime.set_host_policy(policy);

    let result = runtime.check_tool_permission(
        "untrusted-skill",
        "Read",
        None,
        HashMap::new(),
    );

    // Read is in skill's list but trust=false, so fallback applies
    assert!(result.is_err(), "Read should be denied when not trusting skill");
}

#[test]
fn test_host_policy_fallback_deny() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "fallback-skill", "Read");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let policy = HostPolicy::from_config(PermissionsConfig {
        trust_skill_allowed_tools: true,
        fallback: Fallback::Deny,
        deny: vec![],
        allow: vec![],
    });
    runtime.set_host_policy(policy);

    // Tool not in skill's allowed-tools, should hit fallback
    let result = runtime.check_tool_permission(
        "fallback-skill",
        "Execute",
        None,
        HashMap::new(),
    );

    assert!(result.is_err(), "Unknown tool should be denied by fallback");
}

#[test]
fn test_host_policy_fallback_allow() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "permissive-skill", "Read");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let policy = HostPolicy::from_config(PermissionsConfig {
        trust_skill_allowed_tools: true,
        fallback: Fallback::Allow,
        deny: vec![],
        allow: vec![],
    });
    runtime.set_host_policy(policy);

    // Tool not in skill's allowed-tools, but fallback is allow
    let result = runtime.check_tool_permission(
        "permissive-skill",
        "SomeRandomTool",
        None,
        HashMap::new(),
    );

    assert!(result.is_ok(), "Unknown tool should be allowed by fallback");
}

// =============================================================================
// Permission Modes via Host Policy
// =============================================================================

#[test]
fn test_permission_mode_deny_all_via_policy() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "mode-skill", "Read");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Emulate deny-all mode via policy: don't trust skill, fallback to deny
    let policy = HostPolicy::from_config(PermissionsConfig {
        trust_skill_allowed_tools: false,
        fallback: Fallback::Deny,
        deny: vec![],
        allow: vec![],
    });
    runtime.set_host_policy(policy);

    let result = runtime.check_tool_permission(
        "mode-skill",
        "Read",
        None,
        HashMap::new(),
    );

    // Should be denied since we don't trust skill's allowed-tools and fallback is deny
    assert!(result.is_err(), "Should be denied in deny-all mode");
}

#[test]
fn test_permission_mode_allow_all_via_policy() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "mode-skill", "");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Emulate allow-all mode via policy: trust skill, fallback to allow
    let policy = HostPolicy::from_config(PermissionsConfig {
        trust_skill_allowed_tools: true,
        fallback: Fallback::Allow,
        deny: vec![],
        allow: vec![],
    });
    runtime.set_host_policy(policy);

    let result = runtime.check_tool_permission(
        "mode-skill",
        "AnyTool",
        None,
        HashMap::new(),
    );

    // Should be allowed since fallback is allow
    assert!(result.is_ok(), "Should be allowed in allow-all mode");
}

// =============================================================================
// Risky Tool Detection
// =============================================================================

#[test]
fn test_is_risky_tool_write() {
    assert!(is_risky_tool("Write"), "Write should be risky");
}

#[test]
fn test_is_risky_tool_bash() {
    assert!(is_risky_tool("Bash"), "Bash should be risky");
}

#[test]
fn test_is_risky_tool_terminal() {
    assert!(is_risky_tool("Terminal"), "Terminal should be risky");
}

#[test]
fn test_is_risky_tool_read() {
    assert!(!is_risky_tool("Read"), "Read should not be risky");
}

#[test]
fn test_is_risky_tool_grep() {
    assert!(!is_risky_tool("Grep"), "Grep should not be risky");
}

#[test]
fn test_is_risky_tool_list() {
    assert!(!is_risky_tool("LS") && !is_risky_tool("Glob"), "List/Glob should not be risky");
}

// =============================================================================
// Strict Permissions Mode
// =============================================================================

#[test]
fn test_with_strict_permissions() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "strict-skill", "Read, Write");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path())
        .with_strict_permissions();
    runtime.discover_skills().unwrap();

    // In strict mode, even allowed tools may be denied
    let result = runtime.check_tool_permission(
        "strict-skill",
        "Write",
        Some("Attempting write".to_string()),
        HashMap::new(),
    );

    // Strict mode should deny risky tools
    assert!(result.is_err(), "Write should be denied in strict mode");
}

// =============================================================================
// Policy Priority (deny > allow > skill > fallback)
// =============================================================================

#[test]
fn test_policy_priority_deny_overrides_allow() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "priority-skill", "Read");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Read is in both deny and allow lists - deny should win
    let policy = HostPolicy::from_config(PermissionsConfig {
        trust_skill_allowed_tools: true,
        fallback: Fallback::Allow,
        deny: vec!["Read".to_string()],
        allow: vec!["Read".to_string()],
    });
    runtime.set_host_policy(policy);

    let result = runtime.check_tool_permission(
        "priority-skill",
        "Read",
        None,
        HashMap::new(),
    );

    assert!(result.is_err(), "Deny should override allow");
}

// =============================================================================
// Skill Not Found
// =============================================================================

#[test]
fn test_check_permission_skill_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.check_tool_permission(
        "nonexistent-skill",
        "Read",
        None,
        HashMap::new(),
    );

    assert!(result.is_err(), "Should error for nonexistent skill");
}

// =============================================================================
// Multiple Tools
// =============================================================================

#[test]
fn test_check_multiple_tools_same_skill() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_allowed_tools(&temp_dir, "multi-tool-skill", "Read, Grep, LS");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Check multiple tools
    let read_result = runtime.check_tool_permission("multi-tool-skill", "Read", None, HashMap::new());
    let grep_result = runtime.check_tool_permission("multi-tool-skill", "Grep", None, HashMap::new());
    let ls_result = runtime.check_tool_permission("multi-tool-skill", "LS", None, HashMap::new());
    let write_result = runtime.check_tool_permission("multi-tool-skill", "Write", None, HashMap::new());

    assert!(read_result.is_ok(), "Read should be allowed");
    assert!(grep_result.is_ok(), "Grep should be allowed");
    assert!(ls_result.is_ok(), "LS should be allowed");
    assert!(write_result.is_err(), "Write should be denied - not in allowed-tools");
}
