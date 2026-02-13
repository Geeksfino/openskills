//! System Prompt Generation Tests
//!
//! Tests for the getAgentSystemPrompt() function output correctness.
//! Verifies that the generated system prompt includes all necessary sections,
//! skill listings, tool instructions, and proper formatting.

use openskills_runtime::OpenSkillRuntime;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Basic System Prompt Structure
// =============================================================================

#[test]
fn test_system_prompt_empty_registry() {
    let temp_dir = TempDir::new().unwrap();
    // No skills in directory

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    assert!(
        prompt.contains("No skills") || prompt.is_empty() || prompt.len() < 100,
        "Empty registry should produce minimal prompt"
    );
}

#[test]
fn test_system_prompt_contains_available_skills_section() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
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

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    assert!(
        prompt.contains("## Available Skills") || prompt.contains("Available Skills"),
        "Prompt should have Available Skills section"
    );
}

#[test]
fn test_system_prompt_contains_how_to_use_section() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
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

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    assert!(
        prompt.contains("## How to Use") || prompt.contains("How to Use Skills"),
        "Prompt should have How to Use section"
    );
}

#[test]
fn test_system_prompt_contains_available_tools_list() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
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

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should list the standard tools
    assert!(
        prompt.contains("list_skills") || prompt.contains("activate_skill"),
        "Prompt should list available tools"
    );
    assert!(
        prompt.contains("read_skill_file") || prompt.contains("run_skill_script"),
        "Prompt should list skill file tools"
    );
}

// =============================================================================
// Skill Listing Format
// =============================================================================

#[test]
fn test_system_prompt_skill_description_format() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("formatted-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: formatted-skill
description: This is the skill description for testing format.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should format as "- **skill-id**: description"
    assert!(
        prompt.contains("**formatted-skill**") || prompt.contains("formatted-skill"),
        "Prompt should include skill ID"
    );
    assert!(
        prompt.contains("This is the skill description"),
        "Prompt should include skill description"
    );
}

#[test]
fn test_system_prompt_multiple_skills_listed() {
    let temp_dir = TempDir::new().unwrap();

    for name in ["skill-a", "skill-b", "skill-c"] {
        let skill_dir = temp_dir.path().join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                r#"---
name: {}
description: Description for {}.
---
"#,
                name, name
            ),
        )
        .unwrap();
    }

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // All skills should be listed
    assert!(prompt.contains("skill-a"), "Should list skill-a");
    assert!(prompt.contains("skill-b"), "Should list skill-b");
    assert!(prompt.contains("skill-c"), "Should list skill-c");
}

#[test]
fn test_system_prompt_requires_note_appended() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("deps-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: deps-skill
description: Skill with dependencies.
requires:
  bins:
    - git
    - docker
  env:
    - API_KEY
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should include "(requires: ...)" notation
    assert!(
        prompt.contains("(requires:") || prompt.contains("requires:"),
        "Prompt should show requires notation"
    );
    assert!(
        prompt.contains("git") && prompt.contains("API_KEY"),
        "Requires note should include dependency names"
    );
}

// =============================================================================
// Instruction Sections
// =============================================================================

#[test]
fn test_system_prompt_activate_skill_instructions() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: Test.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should instruct agent to activate skills
    assert!(
        prompt.contains("activate_skill") || prompt.contains("Activate"),
        "Should mention skill activation"
    );
}

#[test]
fn test_system_prompt_run_script_instructions() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: Test.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should include script execution guidance
    assert!(
        prompt.contains("run_skill_script") || prompt.contains("WASM"),
        "Should mention script/WASM execution"
    );
}

#[test]
fn test_system_prompt_workspace_instructions() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: Test.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should mention workspace directory
    assert!(
        prompt.contains("workspace") || prompt.contains("SKILL_WORKSPACE"),
        "Should mention workspace directory"
    );
}

// =============================================================================
// Code Generation Section
// =============================================================================

#[test]
fn test_system_prompt_code_generation_section() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: Test.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should include code generation guidance
    assert!(
        prompt.contains("Code Generation") || prompt.contains("CommonJS") || prompt.contains("ES modules"),
        "Should include code generation instructions"
    );
}

// =============================================================================
// Important Notes Section
// =============================================================================

#[test]
fn test_system_prompt_important_notes() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: Test.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should include important notes about not assuming prior knowledge
    assert!(
        prompt.contains("Important") || prompt.contains("do NOT assume"),
        "Should include important notes section"
    );
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_system_prompt_skill_with_special_characters_in_description() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("special-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: special-skill
description: "Description with special chars: <>&\"' and unicode: 日本語"
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should handle special characters gracefully
    assert!(
        prompt.contains("special-skill"),
        "Should include skill despite special chars"
    );
}

#[test]
fn test_system_prompt_skill_with_long_description() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("long-desc-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    let long_desc = "A ".repeat(500); // Near max length
    fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            r#"---
name: long-desc-skill
description: {}
---
"#,
            long_desc.trim()
        ),
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should handle long descriptions
    assert!(
        prompt.contains("long-desc-skill"),
        "Should include skill with long description"
    );
}

// =============================================================================
// File Output Instructions
// =============================================================================

#[test]
fn test_system_prompt_file_output_section() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: Test.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let prompt = runtime.get_agent_system_prompt();

    // Should include file output guidance
    assert!(
        prompt.contains("File Output") || prompt.contains("list_workspace_files") || prompt.contains("get_file_info"),
        "Should include file output instructions"
    );
}
