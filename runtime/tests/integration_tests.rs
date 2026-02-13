//! Integration Tests
//!
//! End-to-end integration tests that verify complete workflows
//! from skill discovery through execution and audit.

use openskills_runtime::{ExecutionContext, OpenSkillRuntime, RuntimeExecutionStatus};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Full Skill Lifecycle
// =============================================================================

#[test]
fn test_full_skill_lifecycle_discover_list_activate() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("lifecycle-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: lifecycle-skill
description: Test full lifecycle.
allowed-tools: Read, Write
---
# Instructions

Follow these steps to complete the task.
"#,
    )
    .unwrap();

    // 1. Create runtime
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());

    // 2. Discover skills
    let discovered = runtime.discover_skills().unwrap();
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].id, "lifecycle-skill");

    // 3. List skills (should be same as discovered)
    let listed = runtime.list_skills();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "lifecycle-skill");

    // 4. Activate skill (load full content)
    let loaded = runtime.activate_skill("lifecycle-skill").unwrap();
    assert_eq!(loaded.id, "lifecycle-skill");
    assert!(loaded.instructions.contains("Follow these steps"));
    assert_eq!(loaded.manifest.get_allowed_tools(), vec!["Read", "Write"]);

    // 5. Verify system prompt includes skill
    let prompt = runtime.get_agent_system_prompt();
    assert!(prompt.contains("lifecycle-skill"));
}

#[test]
fn test_multiple_skills_workflow() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple skills
    for (name, desc) in [
        ("skill-alpha", "First skill"),
        ("skill-beta", "Second skill"),
        ("skill-gamma", "Third skill"),
    ] {
        let skill_dir = temp_dir.path().join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                r#"---
name: {}
description: {}
---
# {} Instructions
"#,
                name, desc, name
            ),
        )
        .unwrap();
    }

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let skills = runtime.discover_skills().unwrap();

    assert_eq!(skills.len(), 3);

    // Activate each skill and verify
    for name in ["skill-alpha", "skill-beta", "skill-gamma"] {
        let loaded = runtime.activate_skill(name).unwrap();
        assert_eq!(loaded.id, name);
        assert!(loaded.instructions.contains(&format!("{} Instructions", name)));
    }
}

// =============================================================================
// Discovery to Prompt Generation
// =============================================================================

#[test]
fn test_discovery_to_prompt_generation() {
    let temp_dir = TempDir::new().unwrap();

    // Create skills with different configurations
    let invocable_dir = temp_dir.path().join("invocable-skill");
    fs::create_dir_all(&invocable_dir).unwrap();
    fs::write(
        invocable_dir.join("SKILL.md"),
        r#"---
name: invocable-skill
description: User-invocable skill.
user-invocable: true
requires:
  bins:
    - git
---
"#,
    )
    .unwrap();

    let always_dir = temp_dir.path().join("always-skill");
    fs::create_dir_all(&always_dir).unwrap();
    fs::write(
        always_dir.join("SKILL.md"),
        r#"---
name: always-skill
description: Always-loaded skill.
user-invocable: false
---
# Always Active

These rules are always active.
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Invocable skill should be in Available Skills
    assert!(
        prompt.contains("invocable-skill"),
        "Invocable skill should be in prompt"
    );

    // Always skill's instructions should be pre-loaded
    assert!(
        prompt.contains("Always Active") || prompt.contains("always-skill"),
        "Always skill should be pre-loaded"
    );

    // Requires should be shown
    assert!(
        prompt.contains("git") || prompt.contains("requires"),
        "Requirements should be shown"
    );
}

// =============================================================================
// Skill Session Workflow
// =============================================================================

#[test]
fn test_skill_session_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("session-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: session-skill
description: Test skill session.
context: fork
allowed-tools: Read, Grep
---
# Session Instructions
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Start session
    let parent = ExecutionContext::new();
    let mut session = runtime
        .start_skill_session("session-skill", Some(json!({"query": "test"})), Some(&parent))
        .unwrap();

    assert!(session.is_forked(), "Session should be forked");

    // Record tool calls
    session.record_tool_call("Read", &json!({"path": "file.txt"}));
    session.record_tool_call("Grep", &json!({"pattern": "test"}));

    // Finish session
    let result = runtime
        .finish_skill_session(
            session,
            json!({"result": "found matches"}),
            String::new(),
            String::new(),
            RuntimeExecutionStatus::Success,
        )
        .unwrap();

    // Verify audit
    assert!(result.audit.permissions_used.contains(&"Read".to_string()));
    assert!(result.audit.permissions_used.contains(&"Grep".to_string()));
}

// =============================================================================
// Dependency Checking in Workflow
// =============================================================================

#[test]
fn test_requires_checked_on_activation() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("deps-workflow-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    // Remove test env var to ensure it's missing
    std::env::remove_var("__INTEGRATION_TEST_TOKEN__");

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: deps-workflow-skill
description: Skill with dependencies for workflow test.
requires:
  bins:
    - sh
    - __missing_integration_tool__
  env:
    - PATH
    - __INTEGRATION_TEST_TOKEN__
---
# Instructions
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Activation should check dependencies
    let loaded = runtime.activate_skill("deps-workflow-skill").unwrap();

    // Should have missing dependencies
    assert!(loaded.missing_dependencies.is_some());
    let missing = loaded.missing_dependencies.unwrap();

    // __missing_integration_tool__ should be missing
    assert!(missing.bins.contains(&"__missing_integration_tool__".to_string()));

    // __INTEGRATION_TEST_TOKEN__ should be missing
    assert!(missing.env.contains(&"__INTEGRATION_TEST_TOKEN__".to_string()));

    // sh and PATH should NOT be missing
    assert!(!missing.bins.contains(&"sh".to_string()));
    assert!(!missing.env.contains(&"PATH".to_string()));
}

// =============================================================================
// File Operations Workflow
// =============================================================================

#[test]
fn test_file_operations_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("file-workflow-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::create_dir_all(skill_dir.join("docs")).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: file-workflow-skill
description: Skill with files.
---
# Instructions

See docs/guide.md for details.
"#,
    )
    .unwrap();

    fs::write(
        skill_dir.join("docs").join("guide.md"),
        "# Guide\n\nDetailed instructions here.",
    )
    .unwrap();

    fs::write(skill_dir.join("config.json"), r#"{"setting": "value"}"#).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Activate skill
    let loaded = runtime.activate_skill("file-workflow-skill").unwrap();
    assert!(loaded.instructions.contains("docs/guide.md"));

    // List files
    let files = runtime
        .list_skill_files("file-workflow-skill", None, true)
        .unwrap();
    assert!(files.iter().any(|f| f.contains("guide.md")));
    assert!(files.iter().any(|f| f.contains("config.json")));

    // Read helper file
    let guide_content = runtime
        .read_skill_file("file-workflow-skill", "docs/guide.md")
        .unwrap();
    assert!(guide_content.contains("Detailed instructions"));

    // Read config
    let config_content = runtime
        .read_skill_file("file-workflow-skill", "config.json")
        .unwrap();
    assert!(config_content.contains("setting"));
}

// =============================================================================
// Permission Workflow
// =============================================================================

#[test]
fn test_permission_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("perm-workflow-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: perm-workflow-skill
description: Permission workflow test.
allowed-tools: Read, Grep
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Check allowed tool
    let read_result = runtime.check_tool_permission(
        "perm-workflow-skill",
        "Read",
        None,
        std::collections::HashMap::new(),
    );
    assert!(read_result.is_ok(), "Read should be allowed");

    // Check disallowed tool
    let write_result = runtime.check_tool_permission(
        "perm-workflow-skill",
        "Write",
        None,
        std::collections::HashMap::new(),
    );
    assert!(write_result.is_err(), "Write should be denied");
}

// =============================================================================
// Runtime Configuration Workflow
// =============================================================================

#[test]
fn test_runtime_config_workflow() {
    use openskills_runtime::RuntimeConfig;

    let temp_dir = TempDir::new().unwrap();
    let custom_dir = temp_dir.path().join("custom-skills");
    fs::create_dir_all(&custom_dir).unwrap();

    let skill_dir = custom_dir.join("config-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: config-skill
description: Custom directory skill.
---
"#,
    )
    .unwrap();

    // Create runtime with custom config
    let config = RuntimeConfig {
        custom_directories: vec![custom_dir.clone()],
        use_standard_locations: false,
        project_root: None,
        workspace_dir: Some(temp_dir.path().join("workspace")),
    };

    let mut runtime = OpenSkillRuntime::from_config(config);
    let skills = runtime.discover_skills().unwrap();

    assert!(skills.iter().any(|s| s.id == "config-skill"));
}

// =============================================================================
// Error Recovery Workflow
// =============================================================================

#[test]
fn test_error_recovery_workflow() {
    let temp_dir = TempDir::new().unwrap();

    // Create one valid skill
    let valid_dir = temp_dir.path().join("valid-skill");
    fs::create_dir_all(&valid_dir).unwrap();
    fs::write(
        valid_dir.join("SKILL.md"),
        r#"---
name: valid-skill
description: Valid skill.
---
"#,
    )
    .unwrap();

    // Create one invalid skill (bad YAML)
    let invalid_dir = temp_dir.path().join("invalid-skill");
    fs::create_dir_all(&invalid_dir).unwrap();
    fs::write(
        invalid_dir.join("SKILL.md"),
        r#"---
name: invalid-skill
description: "Unclosed quote
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let skills = runtime.discover_skills().unwrap();

    // Valid skill should be discovered
    assert!(skills.iter().any(|s| s.id == "valid-skill"));

    // Invalid skill should be skipped
    assert!(!skills.iter().any(|s| s.id == "invalid-skill"));

    // Valid skill should still work
    let loaded = runtime.activate_skill("valid-skill").unwrap();
    assert_eq!(loaded.id, "valid-skill");
}

// =============================================================================
// Concurrent Access (Basic)
// =============================================================================

#[test]
fn test_multiple_activations_same_skill() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("concurrent-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: concurrent-skill
description: Test concurrent access.
---
# Instructions
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Activate same skill multiple times
    let loaded1 = runtime.activate_skill("concurrent-skill").unwrap();
    let loaded2 = runtime.activate_skill("concurrent-skill").unwrap();
    let loaded3 = runtime.activate_skill("concurrent-skill").unwrap();

    // All should succeed with same content
    assert_eq!(loaded1.id, loaded2.id);
    assert_eq!(loaded2.id, loaded3.id);
    assert_eq!(loaded1.instructions, loaded2.instructions);
}

// =============================================================================
// Metadata Consistency
// =============================================================================

#[test]
fn test_metadata_consistency_across_apis() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("metadata-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: metadata-skill
description: Test metadata consistency.
allowed-tools: Read, Write
user-invocable: true
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let discovered = runtime.discover_skills().unwrap();
    let listed = runtime.list_skills();
    let loaded = runtime.activate_skill("metadata-skill").unwrap();

    // IDs should match
    assert_eq!(discovered[0].id, listed[0].id);
    assert_eq!(listed[0].id, loaded.id);

    // Descriptions should match
    assert_eq!(discovered[0].description, listed[0].description);

    // user_invocable should be consistent
    assert_eq!(discovered[0].user_invocable, listed[0].user_invocable);
    assert!(loaded.manifest.is_user_invocable());
}
