use openskills_runtime::OpenSkillRuntime;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_skill_name_max_length() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("x".repeat(64));
    fs::create_dir_all(&skill_dir).unwrap();
    
    let skill_md = format!(
        r#"---
name: {}
description: Test skill with max length name.
---
"#,
        "x".repeat(64)
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let result = runtime.discover_skills();
    assert!(result.is_ok());
}

#[test]
fn test_skill_name_exceeds_max_length() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("x".repeat(65));
    fs::create_dir_all(&skill_dir).unwrap();
    
    let skill_md = format!(
        r#"---
name: {}
description: Test skill with name exceeding max length.
---
"#,
        "x".repeat(65)
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let result = runtime.discover_skills();
    // Should handle gracefully - invalid skills are skipped
    assert!(result.is_ok());
    let skills = result.unwrap();
    // Invalid skill should not be in the list
    assert!(!skills.iter().any(|s| s.id.len() > 64));
}

#[test]
fn test_skill_description_max_length() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    let max_desc = "x".repeat(1024);
    let skill_md = format!(
        r#"---
name: test-skill
description: {}
---
"#,
        max_desc
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let result = runtime.discover_skills();
    assert!(result.is_ok());
}

#[test]
fn test_skill_description_exceeds_max_length() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    let too_long_desc = "x".repeat(1025);
    let skill_md = format!(
        r#"---
name: test-skill
description: {}
---
"#,
        too_long_desc
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let result = runtime.discover_skills();
    // Should handle gracefully - invalid skills are skipped
    assert!(result.is_ok());
    let skills = result.unwrap();
    // Invalid skill should not be in the list
    assert!(!skills.iter().any(|s| s.id == "test-skill"));
}

#[test]
fn test_skill_description_special_characters() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    // Description with special characters (allowed)
    let skill_md = r#"---
name: test-skill
description: "Test with special chars: !@#$%^&*() and unicode: 测试技能"
---
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let result = runtime.discover_skills().unwrap();
    let skill = result.iter().find(|s| s.id == "test-skill").unwrap();
    assert!(skill.description.contains("special chars"));
    assert!(skill.description.contains("测试技能"));
}

#[test]
fn test_skill_empty_instructions_body() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    let skill_md = r#"---
name: test-skill
description: Test skill with empty body.
---
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();
    
    let loaded = runtime.activate_skill("test-skill").unwrap();
    assert_eq!(loaded.instructions.trim(), "");
}

#[test]
fn test_skill_very_large_instructions() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    let large_body = format!("{}{}", "# Instructions\n\n", "x".repeat(100000));
    let skill_md = format!(
        r#"---
name: test-skill
description: Test skill with very large instructions.
---
{}"#,
        large_body
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();
    
    // Should only load metadata at discovery
    let skills = runtime.list_skills();
    assert!(skills.iter().any(|s| s.id == "test-skill"));
    
    // Should load full body on activation
    let loaded = runtime.activate_skill("test-skill").unwrap();
    assert!(loaded.instructions.len() > 100000);
}

#[test]
fn test_skill_minimal_valid() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("a");
    fs::create_dir_all(&skill_dir).unwrap();
    
    // Minimal valid skill (1 char name, minimal description)
    let skill_md = r#"---
name: a
description: b
---
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let result = runtime.discover_skills().unwrap();
    assert!(result.iter().any(|s| s.id == "a"));
}

#[test]
fn test_skill_allowed_tools_empty() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    // Skill with no allowed-tools (all tools allowed)
    let skill_md = r#"---
name: test-skill
description: Test skill with no allowed-tools.
---
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();
    
    let loaded = runtime.activate_skill("test-skill").unwrap();
    let tools = loaded.manifest.get_allowed_tools();
    assert_eq!(tools.len(), 0); // Empty list means all tools allowed
}

#[test]
fn test_skill_allowed_tools_yaml_list() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    // Skill with YAML list format for allowed-tools
    let skill_md = r#"---
name: test-skill
description: Test skill with YAML list tools.
allowed-tools:
  - Read
  - Write
  - Bash
---
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();
    
    let loaded = runtime.activate_skill("test-skill").unwrap();
    let tools = loaded.manifest.get_allowed_tools();
    assert_eq!(tools.len(), 3);
    assert!(tools.contains(&"Read".to_string()));
    assert!(tools.contains(&"Write".to_string()));
    assert!(tools.contains(&"Bash".to_string()));
}
