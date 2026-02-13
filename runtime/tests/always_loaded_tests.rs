//! Always-Loaded Skills Tests
//!
//! Tests for OpenClaw-compatible always-loaded skills functionality.
//! Skills with `user-invocable: false` should have their full instructions
//! automatically pre-loaded into the system prompt.

use openskills_runtime::OpenSkillRuntime;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Basic Always-Loaded Behavior
// =============================================================================

#[test]
fn test_always_skill_instructions_in_system_prompt() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("always-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = r#"---
name: always-skill
description: An always-loaded skill.
user-invocable: false
---
# Always Active Instructions

These instructions should appear in the system prompt automatically.
Do not require activation.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should contain the "Pre-loaded Skills" section
    assert!(
        prompt.contains("Pre-loaded Skill") || prompt.contains("Pre-loaded"),
        "Prompt should have pre-loaded skills section: {}",
        prompt
    );

    // Should contain the skill's full instructions
    assert!(
        prompt.contains("Always Active Instructions"),
        "Prompt should contain always-skill instructions: {}",
        prompt
    );
    assert!(
        prompt.contains("Do not require activation"),
        "Prompt should contain full instruction body"
    );
}

#[test]
fn test_always_skill_not_in_available_skills_list() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("hidden-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = r#"---
name: hidden-skill
description: This skill is always active, not shown in list.
user-invocable: false
---
# Hidden Instructions
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // The "Available Skills" section should NOT list this skill by ID for activation
    // (It may still be mentioned in the pre-loaded section)
    let available_section = prompt
        .split("## Available Skills")
        .nth(1)
        .and_then(|s| s.split("## How to Use").next())
        .unwrap_or("");

    // In the available skills section, the skill should not appear as "- **hidden-skill**:"
    assert!(
        !available_section.contains("- **hidden-skill**:"),
        "Always-skill should not appear in Available Skills as activatable"
    );
}

// =============================================================================
// Mixed Skills (Always + Invocable)
// =============================================================================

#[test]
fn test_mixed_always_and_invocable_skills() {
    let temp_dir = TempDir::new().unwrap();

    // Create an always-loaded skill
    let always_dir = temp_dir.path().join("always-active");
    fs::create_dir_all(&always_dir).unwrap();
    fs::write(
        always_dir.join("SKILL.md"),
        r#"---
name: always-active
description: Always active skill.
user-invocable: false
---
# Core Rules
Follow these rules at all times.
"#,
    )
    .unwrap();

    // Create a user-invocable skill
    let invocable_dir = temp_dir.path().join("on-demand");
    fs::create_dir_all(&invocable_dir).unwrap();
    fs::write(
        invocable_dir.join("SKILL.md"),
        r#"---
name: on-demand
description: Activate when needed.
user-invocable: true
---
# On Demand Instructions
Only activate when user requests.
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Invocable skill should appear in Available Skills
    assert!(
        prompt.contains("**on-demand**") || prompt.contains("on-demand"),
        "On-demand skill should appear in prompt"
    );

    // Always skill's instructions should be pre-loaded
    assert!(
        prompt.contains("Core Rules") && prompt.contains("Follow these rules"),
        "Always-active skill instructions should be pre-loaded"
    );
}

#[test]
fn test_multiple_always_skills() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple always-loaded skills
    for (name, instructions) in [
        ("always-1", "First always-loaded skill instructions."),
        ("always-2", "Second always-loaded skill instructions."),
        ("always-3", "Third always-loaded skill instructions."),
    ] {
        let skill_dir = temp_dir.path().join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                r#"---
name: {}
description: Always skill {}.
user-invocable: false
---
# Instructions
{}
"#,
                name, name, instructions
            ),
        )
        .unwrap();
    }

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // All always-skills should have their instructions pre-loaded
    assert!(
        prompt.contains("First always-loaded skill instructions"),
        "First always-skill should be pre-loaded"
    );
    assert!(
        prompt.contains("Second always-loaded skill instructions"),
        "Second always-skill should be pre-loaded"
    );
    assert!(
        prompt.contains("Third always-loaded skill instructions"),
        "Third always-skill should be pre-loaded"
    );
}

#[test]
fn test_all_skills_always_loaded_message() {
    let temp_dir = TempDir::new().unwrap();

    // Create only always-loaded skills (no user-invocable ones)
    let skill_dir = temp_dir.path().join("only-always");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: only-always
description: The only skill, always active.
user-invocable: false
---
# The Only Skill
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should indicate there are no skills to activate
    assert!(
        prompt.contains("No additional skills to activate")
            || prompt.contains("always active")
            || prompt.contains("Pre-loaded"),
        "Should indicate skills are pre-loaded, not activatable: {}",
        prompt
    );
}

// =============================================================================
// Always-Loaded with Dependencies
// =============================================================================

#[test]
fn test_always_skill_with_requires() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("always-with-deps");
    fs::create_dir_all(&skill_dir).unwrap();

    std::env::remove_var("__MISSING_API_KEY__");

    let skill_md = r#"---
name: always-with-deps
description: Always-loaded skill with dependencies.
user-invocable: false
requires:
  bins:
    - __nonexistent_tool__
  env:
    - __MISSING_API_KEY__
---
# Instructions With Dependencies
Requires specific tools and env vars.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Even though it's always-loaded, activation should still check deps
    let loaded = runtime.activate_skill("always-with-deps").unwrap();

    assert!(loaded.missing_dependencies.is_some());
    let missing = loaded.missing_dependencies.unwrap();
    assert!(!missing.bins.is_empty() || !missing.env.is_empty());
}

// =============================================================================
// userInvocable Flag Behavior
// =============================================================================

#[test]
fn test_user_invocable_defaults_to_true() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("default-invocable");
    fs::create_dir_all(&skill_dir).unwrap();

    // No user-invocable field specified
    let skill_md = r#"---
name: default-invocable
description: Should default to user-invocable true.
---
# Instructions
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let skills = runtime.discover_skills().unwrap();

    let skill = skills.iter().find(|s| s.id == "default-invocable").unwrap();
    assert!(
        skill.user_invocable,
        "user_invocable should default to true"
    );
}

#[test]
fn test_user_invocable_explicit_true() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("explicit-true");
    fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = r#"---
name: explicit-true
description: Explicitly user-invocable.
user-invocable: true
---
# Instructions
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let skills = runtime.discover_skills().unwrap();

    let skill = skills.iter().find(|s| s.id == "explicit-true").unwrap();
    assert!(skill.user_invocable, "user_invocable should be true");
}

#[test]
fn test_user_invocable_explicit_false() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("explicit-false");
    fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = r#"---
name: explicit-false
description: Explicitly not user-invocable.
user-invocable: false
---
# Instructions
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let skills = runtime.discover_skills().unwrap();

    let skill = skills.iter().find(|s| s.id == "explicit-false").unwrap();
    assert!(!skill.user_invocable, "user_invocable should be false");
}

// =============================================================================
// List Skills Filtering
// =============================================================================

#[test]
fn test_list_skills_includes_always_skills() {
    let temp_dir = TempDir::new().unwrap();

    // Create an always-loaded skill
    let skill_dir = temp_dir.path().join("listed-always");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: listed-always
description: Always skill that should appear in list.
user-invocable: false
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let skills = runtime.list_skills();

    // listSkills should include always-loaded skills (they're still discoverable)
    let skill = skills.iter().find(|s| s.id == "listed-always");
    assert!(skill.is_some(), "listSkills should include always-loaded skills");
    assert!(!skill.unwrap().user_invocable);
}
