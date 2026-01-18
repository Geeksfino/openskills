use openskills_runtime::OpenSkillRuntime;
use std::fs;
use tempfile::TempDir;

fn write_skill(dir: &std::path::Path, name: &str, allowed_tools: &str) {
    let skill_dir = dir.join(name);
    fs::create_dir(&skill_dir).unwrap();
    let manifest = format!(
        r#"---
name: {name}
description: Test skill
allowed-tools: {allowed_tools}
---
"#
    );
    fs::write(skill_dir.join("SKILL.md"), manifest).unwrap();
}

#[test]
fn test_check_tool_permission_denies_unlisted_tool() {
    let temp_dir = TempDir::new().unwrap();
    write_skill(temp_dir.path(), "perm-test-skill", "Read");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.check_tool_permission(
        "perm-test-skill",
        "Write",
        None,
        std::collections::HashMap::new(),
    );

    assert!(result.is_err(), "unlisted tool should be denied");
}

#[test]
fn test_check_tool_permission_risky_tool_denied_by_callback() {
    let temp_dir = TempDir::new().unwrap();
    write_skill(temp_dir.path(), "perm-risky-skill", "Write");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path()).with_strict_permissions();
    runtime.discover_skills().unwrap();

    let allowed = runtime
        .check_tool_permission(
            "perm-risky-skill",
            "Write",
            Some("Attempt write".to_string()),
            std::collections::HashMap::new(),
        )
        .expect("permission check should return result");

    assert!(!allowed, "strict permissions should deny risky tool");
}
