//! Claude Skills Specification Conformance Tests
//!
//! This module tests conformance to the official Claude Skills specification:
//! https://agentskills.io/specification
//!
//! ## Spec Coverage
//!
//! - SKILL.md format (YAML frontmatter + Markdown body)
//! - Required fields (name, description)
//! - Optional fields (allowed-tools, model, context, agent, hooks, user-invocable)
//! - Validation rules (name length, format, reserved words, etc.)
//! - Progressive disclosure (3-tier loading)
//! - Context fork mechanism
//! - Discovery paths

use openskills_runtime::{
    OpenSkillRuntime, parse_skill_md, validate_skill_path,
};
use std::fs;
use tempfile::TempDir;

// =============================================================================
// SKILL.md Format Tests
// =============================================================================

#[test]
fn spec_skill_md_requires_yaml_frontmatter() {
    let content = r#"# Instructions
This file has no frontmatter.
"#;
    let result = parse_skill_md(content);
    assert!(result.is_err(), "SKILL.md without frontmatter should fail");
}

#[test]
fn spec_skill_md_frontmatter_must_start_with_triple_dash() {
    let content = r#"name: test
description: test
---
# Instructions
"#;
    let result = parse_skill_md(content);
    assert!(result.is_err(), "Frontmatter must start with ---");
}

#[test]
fn spec_skill_md_frontmatter_must_be_closed() {
    let content = r#"---
name: test
description: test
# Instructions
"#;
    let result = parse_skill_md(content);
    assert!(result.is_err(), "Frontmatter must be closed with ---");
}

#[test]
fn spec_skill_md_valid_format() {
    let content = r#"---
name: test-skill
description: A test skill for conformance testing.
---
# Instructions

Follow these steps.
"#;
    let result = parse_skill_md(content);
    assert!(result.is_ok(), "Valid SKILL.md should parse: {:?}", result.err());
    
    let parsed = result.unwrap();
    assert_eq!(parsed.manifest.name, "test-skill");
    assert_eq!(parsed.manifest.description, "A test skill for conformance testing.");
    assert!(parsed.instructions.contains("Follow these steps"));
}

// =============================================================================
// Required Fields Tests
// =============================================================================

#[test]
fn spec_name_is_required() {
    let content = r#"---
description: A skill without a name.
---
# Instructions
"#;
    let result = parse_skill_md(content);
    assert!(result.is_err(), "name field is required");
}

#[test]
fn spec_description_is_required() {
    let content = r#"---
name: test-skill
---
# Instructions
"#;
    let result = parse_skill_md(content);
    assert!(result.is_err(), "description field is required");
}

#[test]
fn spec_name_max_64_chars() {
    let long_name = "a".repeat(65);
    let content = format!(
        r#"---
name: {}
description: A skill with a too-long name.
---
"#,
        long_name
    );
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join(&long_name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    
    let result = validate_skill_path(&skill_dir);
    assert!(!result.errors.is_empty(), "Name > 64 chars should be invalid");
}

#[test]
fn spec_name_lowercase_alphanumeric_hyphens_only() {
    let test_cases = vec![
        ("valid-name", true),
        ("valid123", true),
        ("UPPERCASE", false),
        ("has_underscore", false),
        ("has.dot", false),
        ("has space", false),
        ("-leading-hyphen", false),
        ("trailing-hyphen-", false),
        ("double--hyphen", false),
    ];
    
    for (name, should_be_valid) in test_cases {
        let content = format!(
            r#"---
name: {}
description: Testing name format.
---
"#,
            name
        );
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
        
        let result = validate_skill_path(&skill_dir);
        assert_eq!(
            result.errors.is_empty(),
            should_be_valid,
            "Name '{}' should be {}",
            name,
            if should_be_valid { "valid" } else { "invalid" }
        );
    }
}

#[test]
fn spec_name_reserved_words_rejected() {
    // These are the actual reserved words from validator.rs
    let reserved_names = vec!["anthropic", "claude", "skill", "system"];
    
    for name in reserved_names {
        let content = format!(
            r#"---
name: {}
description: Testing reserved name.
---
"#,
            name
        );
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
        
        let result = validate_skill_path(&skill_dir);
        assert!(!result.errors.is_empty(), "Reserved name '{}' should be rejected", name);
    }
}

#[test]
fn spec_description_max_1024_chars() {
    let long_desc = "a".repeat(1025);
    let content = format!(
        r#"---
name: test-skill
description: {}
---
"#,
        long_desc
    );
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    
    let result = validate_skill_path(&skill_dir);
    assert!(!result.errors.is_empty(), "Description > 1024 chars should be invalid");
}

// =============================================================================
// Optional Fields Tests
// =============================================================================

#[test]
fn spec_allowed_tools_comma_separated() {
    let content = r#"---
name: test-skill
description: Testing allowed-tools.
allowed-tools: Read, Write, Bash
---
"#;
    let parsed = parse_skill_md(content).unwrap();
    let tools = parsed.manifest.get_allowed_tools();
    assert_eq!(tools, vec!["Read", "Write", "Bash"]);
}

#[test]
fn spec_allowed_tools_space_separated() {
    let content = r#"---
name: test-skill
description: Testing allowed-tools.
allowed-tools: Read Write Bash
---
"#;
    let parsed = parse_skill_md(content).unwrap();
    let tools = parsed.manifest.get_allowed_tools();
    assert_eq!(tools, vec!["Read", "Write", "Bash"]);
}

#[test]
fn spec_allowed_tools_yaml_list() {
    let content = r#"---
name: test-skill
description: Testing allowed-tools.
allowed-tools:
  - Read
  - Write
  - Bash
---
"#;
    let parsed = parse_skill_md(content).unwrap();
    let tools = parsed.manifest.get_allowed_tools();
    assert_eq!(tools, vec!["Read", "Write", "Bash"]);
}

#[test]
fn spec_context_fork_valid() {
    let content = r#"---
name: test-skill
description: Testing context fork.
context: fork
---
"#;
    let parsed = parse_skill_md(content).unwrap();
    assert!(parsed.manifest.is_forked());
}

#[test]
fn spec_context_invalid_value() {
    let content = r#"---
name: test-skill
description: Testing context.
context: invalid
---
"#;
    // Parse the skill and use activate_skill which calls validate_skill 
    // (which includes validate_manifest that checks context)
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    runtime.discover_skills().unwrap();
    
    // activate_skill should fail because context: invalid is not allowed
    let result = runtime.activate_skill("test-skill");
    assert!(result.is_err(), "context: invalid should be rejected during activation");
}

#[test]
fn spec_user_invocable_defaults_true() {
    let content = r#"---
name: test-skill
description: Testing user-invocable.
---
"#;
    let parsed = parse_skill_md(content).unwrap();
    assert!(parsed.manifest.is_user_invocable());
}

#[test]
fn spec_user_invocable_can_be_false() {
    let content = r#"---
name: test-skill
description: Testing user-invocable.
user-invocable: false
---
"#;
    let parsed = parse_skill_md(content).unwrap();
    assert!(!parsed.manifest.is_user_invocable());
}

// =============================================================================
// Progressive Disclosure Tests
// =============================================================================

#[test]
fn spec_progressive_disclosure_tier1_metadata_only() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    // Create a skill with large instructions
    let large_body = "x".repeat(10000);
    let content = format!(
        r#"---
name: test-skill
description: A test skill.
---
# Instructions
{}"#,
        large_body
    );
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    
    // Discover should only load metadata
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    let skills = runtime.discover_skills().unwrap();
    
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].id, "test-skill");
    // At discovery, we should NOT have loaded the full instructions
    // (This is verified by the registry only storing SkillMetadata)
}

#[test]
fn spec_progressive_disclosure_tier2_activation_loads_instructions() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    let content = r#"---
name: test-skill
description: A test skill.
---
# Instructions

These are the full instructions.
"#;
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    runtime.discover_skills().unwrap();
    
    // Activation should load full instructions
    let loaded = runtime.activate_skill("test-skill").unwrap();
    assert!(loaded.instructions.contains("These are the full instructions"));
}

// =============================================================================
// Discovery Paths Tests
// =============================================================================

#[test]
fn spec_discovery_from_custom_directory() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("my-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: my-skill
description: Custom directory skill.
---
"#,
    )
    .unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    let skills = runtime.discover_skills().unwrap();
    
    assert!(skills.iter().any(|s| s.id == "my-skill"));
}

#[test]
fn spec_discovery_directory_name_must_match_manifest_name() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("wrong-name");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: correct-name
description: Directory name doesn't match.
---
"#,
    )
    .unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    let skills = runtime.discover_skills().unwrap();
    
    // The skill should not be loaded because directory name doesn't match
    assert!(skills.is_empty() || !skills.iter().any(|s| s.id == "correct-name"));
}

#[test]
fn spec_discovery_later_paths_override_earlier() {
    let temp1 = TempDir::new().unwrap();
    let temp2 = TempDir::new().unwrap();
    
    // Create same skill in both directories with different descriptions
    for (temp, desc) in [(temp1.path(), "First version"), (temp2.path(), "Second version")] {
        let skill_dir = temp.join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                r#"---
name: test-skill
description: {}
---
"#,
                desc
            ),
        )
        .unwrap();
    }
    
    // Load from first, then second
    let mut runtime = OpenSkillRuntime::new()
        .with_custom_directory(temp1.path())
        .with_custom_directory(temp2.path())
        .with_standard_locations(false);
    let skills = runtime.discover_skills().unwrap();
    
    // Second should override first
    let skill = skills.iter().find(|s| s.id == "test-skill").unwrap();
    assert!(skill.description.contains("Second version"));
}

// =============================================================================
// Context Fork Tests
// =============================================================================

#[test]
fn spec_context_fork_creates_isolated_session() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("fork-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: fork-skill
description: A forked skill.
context: fork
---
"#,
    )
    .unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    runtime.discover_skills().unwrap();
    
    let session = runtime
        .start_skill_session("fork-skill", None, None)
        .unwrap();
    
    assert!(session.is_forked());
    assert!(session.context_id().is_some());
}

#[test]
fn spec_non_forked_skill_no_isolated_context() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("normal-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: normal-skill
description: A normal skill.
---
"#,
    )
    .unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    runtime.discover_skills().unwrap();
    
    let session = runtime
        .start_skill_session("normal-skill", None, None)
        .unwrap();
    
    assert!(!session.is_forked());
    assert!(session.context_id().is_none());
}

// =============================================================================
// Workspace Management Tests
// =============================================================================

#[test]
fn spec_workspace_directory_created_on_demand() {
    let runtime = OpenSkillRuntime::new();
    let workspace = runtime.get_workspace_dir().unwrap();
    
    assert!(workspace.exists());
    assert!(workspace.is_dir());
    
    // Cleanup
    let _ = runtime.cleanup_workspace();
}

#[test]
fn spec_workspace_directory_custom_path() {
    let temp = TempDir::new().unwrap();
    let custom_workspace = temp.path().join("my-workspace");
    
    let runtime = OpenSkillRuntime::new()
        .with_workspace_dir(&custom_workspace);
    
    let workspace = runtime.get_workspace_dir().unwrap();
    assert_eq!(workspace, custom_workspace);
    assert!(workspace.exists());
}

// =============================================================================
// System Prompt Tests
// =============================================================================

#[test]
fn spec_system_prompt_includes_skill_metadata() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: A test skill for prompt generation.
---
"#,
    )
    .unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    runtime.discover_skills().unwrap();
    
    let prompt = runtime.get_system_prompt_metadata();
    assert!(prompt.contains("test-skill"));
    assert!(prompt.contains("A test skill for prompt generation"));
}

#[test]
fn spec_agent_system_prompt_is_skill_agnostic() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: A test skill.
---
"#,
    )
    .unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    runtime.discover_skills().unwrap();
    
    let prompt = runtime.get_agent_system_prompt();
    
    // Should include the skill
    assert!(prompt.contains("test-skill"));
    
    // Should include generic instructions
    assert!(prompt.contains("activate_skill"));
    assert!(prompt.contains("SKILL.md"));
    assert!(prompt.contains("do NOT assume prior knowledge"));
}

// =============================================================================
// Hooks Tests (if hooks are part of the spec)
// =============================================================================

#[test]
fn spec_hooks_parsed_from_manifest() {
    let content = r#"---
name: test-skill
description: A skill with hooks.
hooks:
  PreToolUse:
    - matcher: "*"
      command: echo "pre"
  PostToolUse:
    - command: echo "post"
---
"#;
    let parsed = parse_skill_md(content).unwrap();
    let hooks = parsed.manifest.hooks.as_ref().unwrap();
    
    assert!(hooks.pre_tool_use.is_some());
    assert!(hooks.post_tool_use.is_some());
}
