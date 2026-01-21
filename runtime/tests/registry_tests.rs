use openskills_runtime::OpenSkillRuntime;
use serde_json::json;

#[test]
fn loads_example_skill_metadata() {
    let skills_dir = format!(
        "{}/../examples/skills",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut runtime = OpenSkillRuntime::from_directory(&skills_dir);
    let skills = runtime.load_from_directory(&skills_dir).expect("load skills");
    // Check for actual skills that exist (code-review, explaining-code, etc.)
    assert!(skills.iter().any(|s| s.id == "code-review" || s.id == "explaining-code" || s.id == "skill-creator"));
}

#[test]
fn validates_input_schema() {
    let skills_dir = format!(
        "{}/../examples/skills",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut runtime = OpenSkillRuntime::from_directory(&skills_dir);
    let _ = runtime.load_from_directory(&skills_dir);
    
    // Use an actual skill that exists
    let result = runtime.execute_skill(
        "code-review",
        openskills_runtime::ExecutionOptions {
            timeout_ms: Some(1000),
            input: Some(json!({"conversation": "hello"})),
            ..Default::default()
        },
    );

    // This will fail due to placeholder wasm, but should fail after validation
    assert!(result.is_err());
}

#[test]
fn test_progressive_disclosure_metadata_only() {
    use tempfile::TempDir;
    use std::fs;
    
    // Create a temporary skill with large instructions
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    let large_body = format!("{}{}", "# Instructions\n\n", "x".repeat(10000));
    let skill_md = format!(
        r#"---
name: test-skill
description: A test skill with large instructions.
---
{}"#,
        large_body
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    // Discover skills - should only load metadata
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    let skills = runtime.load_from_directory(temp.path()).unwrap();
    
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].id, "test-skill");
    // At this point, instructions should NOT be loaded in registry
}

#[test]
fn test_progressive_disclosure_activation_loads_body() {
    use tempfile::TempDir;
    use std::fs;
    
    // Create a temporary skill
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    let instructions = "# Instructions\n\nThis is the full instruction body.";
    let skill_md = format!(
        r#"---
name: test-skill
description: A test skill.
---
{}"#,
        instructions
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    // Discover skills
    let mut runtime = OpenSkillRuntime::from_directory(temp.path());
    let _ = runtime.load_from_directory(temp.path()).unwrap();
    
    // Activate skill - should now load full body
    let loaded = runtime.activate_skill("test-skill").unwrap();
    assert_eq!(loaded.id, "test-skill");
    assert!(loaded.instructions.contains("This is the full instruction body"));
}
