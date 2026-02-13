//! Dependency Checking Tests
//!
//! Tests for OpenClaw-compatible requires.bins and requires.env functionality.
//! Verifies that the runtime correctly checks for binary dependencies and
//! environment variables, and reports missing dependencies on skill activation.

use openskills_runtime::OpenSkillRuntime;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Binary Existence Checks
// =============================================================================

#[test]
fn test_activate_skill_with_existing_binary_dep() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("binary-test");
    fs::create_dir_all(&skill_dir).unwrap();

    // 'sh' should exist on all Unix systems
    let skill_md = r#"---
name: binary-test
description: Test skill with existing binary dependency.
requires:
  bins:
    - sh
---
# Instructions
Use shell.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let loaded = runtime.activate_skill("binary-test").unwrap();

    // sh should exist, so no missing dependencies
    assert!(
        loaded.missing_dependencies.is_none()
            || loaded
                .missing_dependencies
                .as_ref()
                .map(|m| m.bins.is_empty())
                .unwrap_or(true),
        "sh should be found in PATH"
    );
}

#[test]
fn test_activate_skill_with_missing_binary_dep() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("missing-bin-test");
    fs::create_dir_all(&skill_dir).unwrap();

    // This binary should definitely not exist
    let skill_md = r#"---
name: missing-bin-test
description: Test skill with missing binary dependency.
requires:
  bins:
    - __nonexistent_binary_xyz_12345__
---
# Instructions
This won't work without the binary.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let loaded = runtime.activate_skill("missing-bin-test").unwrap();

    // Should report missing binary
    assert!(loaded.missing_dependencies.is_some(), "Should have missing dependencies");
    let missing = loaded.missing_dependencies.unwrap();
    assert!(
        missing.bins.contains(&"__nonexistent_binary_xyz_12345__".to_string()),
        "Should report nonexistent binary as missing"
    );
}

#[test]
fn test_activate_skill_with_multiple_binary_deps() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("multi-bin-test");
    fs::create_dir_all(&skill_dir).unwrap();

    // Mix of existing and non-existing binaries
    let skill_md = r#"---
name: multi-bin-test
description: Test skill with multiple binary dependencies.
requires:
  bins:
    - sh
    - __missing_bin_1__
    - cat
    - __missing_bin_2__
---
# Instructions
Needs multiple tools.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let loaded = runtime.activate_skill("multi-bin-test").unwrap();

    // Should report only missing binaries
    assert!(loaded.missing_dependencies.is_some());
    let missing = loaded.missing_dependencies.unwrap();
    assert!(missing.bins.contains(&"__missing_bin_1__".to_string()));
    assert!(missing.bins.contains(&"__missing_bin_2__".to_string()));
    // sh and cat should NOT be in missing list
    assert!(!missing.bins.contains(&"sh".to_string()));
    assert!(!missing.bins.contains(&"cat".to_string()));
}

// =============================================================================
// Environment Variable Checks
// =============================================================================

#[test]
fn test_activate_skill_with_existing_env_dep() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("env-test");
    fs::create_dir_all(&skill_dir).unwrap();

    // PATH is always set
    let skill_md = r#"---
name: env-test
description: Test skill with existing env dependency.
requires:
  env:
    - PATH
---
# Instructions
Use PATH.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let loaded = runtime.activate_skill("env-test").unwrap();

    // PATH should exist, so no missing env
    assert!(
        loaded.missing_dependencies.is_none()
            || loaded
                .missing_dependencies
                .as_ref()
                .map(|m| m.env.is_empty())
                .unwrap_or(true),
        "PATH should be set"
    );
}

#[test]
fn test_activate_skill_with_missing_env_dep() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("missing-env-test");
    fs::create_dir_all(&skill_dir).unwrap();

    // Ensure this env var is not set
    std::env::remove_var("__OPENSKILLS_TEST_MISSING_VAR__");

    let skill_md = r#"---
name: missing-env-test
description: Test skill with missing env dependency.
requires:
  env:
    - __OPENSKILLS_TEST_MISSING_VAR__
---
# Instructions
Needs the env var.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let loaded = runtime.activate_skill("missing-env-test").unwrap();

    // Should report missing env var
    assert!(loaded.missing_dependencies.is_some());
    let missing = loaded.missing_dependencies.unwrap();
    assert!(
        missing.env.contains(&"__OPENSKILLS_TEST_MISSING_VAR__".to_string()),
        "Should report missing env var"
    );
}

#[test]
fn test_activate_skill_with_empty_env_value() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("empty-env-test");
    fs::create_dir_all(&skill_dir).unwrap();

    // Set env var to empty string
    std::env::set_var("__OPENSKILLS_TEST_EMPTY_VAR__", "");

    let skill_md = r#"---
name: empty-env-test
description: Test skill with empty env value.
requires:
  env:
    - __OPENSKILLS_TEST_EMPTY_VAR__
---
# Instructions
Needs non-empty env var.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let loaded = runtime.activate_skill("empty-env-test").unwrap();

    // Empty env var should be considered "missing"
    assert!(loaded.missing_dependencies.is_some());
    let missing = loaded.missing_dependencies.unwrap();
    assert!(
        missing.env.contains(&"__OPENSKILLS_TEST_EMPTY_VAR__".to_string()),
        "Empty env var should be reported as missing"
    );

    // Cleanup
    std::env::remove_var("__OPENSKILLS_TEST_EMPTY_VAR__");
}

// =============================================================================
// Combined Dependencies
// =============================================================================

#[test]
fn test_activate_skill_with_both_bins_and_env() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("combined-deps-test");
    fs::create_dir_all(&skill_dir).unwrap();

    std::env::remove_var("__MISSING_TOKEN__");

    let skill_md = r#"---
name: combined-deps-test
description: Test skill with both binary and env dependencies.
requires:
  bins:
    - sh
    - __missing_tool__
  env:
    - PATH
    - __MISSING_TOKEN__
---
# Instructions
Needs tools and tokens.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let loaded = runtime.activate_skill("combined-deps-test").unwrap();

    assert!(loaded.missing_dependencies.is_some());
    let missing = loaded.missing_dependencies.unwrap();

    // Check bins
    assert!(missing.bins.contains(&"__missing_tool__".to_string()));
    assert!(!missing.bins.contains(&"sh".to_string()));

    // Check env
    assert!(missing.env.contains(&"__MISSING_TOKEN__".to_string()));
    assert!(!missing.env.contains(&"PATH".to_string()));
}

#[test]
fn test_activate_skill_all_deps_satisfied() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("satisfied-deps-test");
    fs::create_dir_all(&skill_dir).unwrap();

    // Set a test env var
    std::env::set_var("__OPENSKILLS_TEST_SET_VAR__", "value");

    let skill_md = r#"---
name: satisfied-deps-test
description: Test skill with all dependencies satisfied.
requires:
  bins:
    - sh
  env:
    - PATH
    - __OPENSKILLS_TEST_SET_VAR__
---
# Instructions
All deps should be satisfied.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let loaded = runtime.activate_skill("satisfied-deps-test").unwrap();

    // No missing dependencies
    assert!(
        loaded.missing_dependencies.is_none()
            || loaded
                .missing_dependencies
                .as_ref()
                .map(|m| m.bins.is_empty() && m.env.is_empty())
                .unwrap_or(true),
        "All dependencies should be satisfied"
    );

    // Cleanup
    std::env::remove_var("__OPENSKILLS_TEST_SET_VAR__");
}

// =============================================================================
// Requires Summary in Skill Descriptor
// =============================================================================

#[test]
fn test_requires_summary_in_list_skills() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("summary-test");
    fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = r#"---
name: summary-test
description: Test skill for requires summary.
requires:
  bins:
    - git
    - docker
  env:
    - GITHUB_TOKEN
---
# Instructions
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let skills = runtime.discover_skills().unwrap();

    let skill = skills.iter().find(|s| s.id == "summary-test").unwrap();

    // Should have a requires_summary field
    assert!(skill.requires_summary.is_some(), "Should have requires_summary");
    let summary = skill.requires_summary.as_ref().unwrap();

    // Summary should mention the dependencies
    assert!(summary.contains("git"), "Summary should mention git");
    assert!(summary.contains("docker"), "Summary should mention docker");
    assert!(summary.contains("GITHUB_TOKEN"), "Summary should mention GITHUB_TOKEN");
}

#[test]
fn test_no_requires_no_summary() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("no-requires-test");
    fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = r#"---
name: no-requires-test
description: Test skill without requires.
---
# Instructions
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let skills = runtime.discover_skills().unwrap();

    let skill = skills.iter().find(|s| s.id == "no-requires-test").unwrap();

    // Should not have a requires_summary
    assert!(skill.requires_summary.is_none(), "Should not have requires_summary when no requires");
}
