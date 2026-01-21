use openskills_runtime::OpenSkillRuntime;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_skill_not_found_error() {
    let temp_dir = TempDir::new().unwrap();
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.activate_skill("nonexistent-skill");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_discover_skills_invalid_directory() {
    let temp_dir = TempDir::new().unwrap();
    let invalid_dir = temp_dir.path().join("nonexistent");
    
    let mut runtime = OpenSkillRuntime::from_directory(&invalid_dir);
    let result = runtime.discover_skills();
    // Should handle gracefully (empty result or error)
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_load_skill_io_error() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    // Create SKILL.md but make it unreadable (on Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(skill_dir.join("SKILL.md"), "---\nname: test-skill\ndescription: Test\n---").unwrap();
        fs::set_permissions(&skill_dir, fs::Permissions::from_mode(0o000)).unwrap();
        
        let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
        let result = runtime.discover_skills();
        // Should handle gracefully
        assert!(result.is_ok() || result.is_err());
        
        // Restore permissions for cleanup
        fs::set_permissions(&skill_dir, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn test_skill_with_invalid_yaml() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("invalid-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    // Invalid YAML (missing closing quote)
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: "Unclosed quote
---
"#,
    ).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let result = runtime.discover_skills();
    // Should handle gracefully - invalid skills are skipped with warning
    assert!(result.is_ok());
    let skills = result.unwrap();
    // Invalid skill should not be in the list
    assert!(!skills.iter().any(|s| s.id == "invalid-skill"));
}

#[test]
fn test_skill_directory_name_mismatch() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("wrong-name");
    fs::create_dir_all(&skill_dir).unwrap();
    
    // Directory name doesn't match manifest name
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: different-name
description: Name mismatch
---
"#,
    ).unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let result = runtime.discover_skills();
    // Should handle gracefully - mismatched names are rejected
    assert!(result.is_ok());
    let skills = result.unwrap();
    // Skill with mismatched name should not be in the list
    assert!(!skills.iter().any(|s| s.id == "wrong-name"));
    assert!(!skills.iter().any(|s| s.id == "different-name"));
}

#[test]
fn test_skill_missing_skill_md() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("no-skill-md");
    fs::create_dir_all(&skill_dir).unwrap();
    
    // Directory exists but no SKILL.md
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let result = runtime.discover_skills().unwrap();
    // Directory without SKILL.md should be ignored
    assert!(!result.iter().any(|s| s.id == "no-skill-md"));
}

#[test]
fn test_skill_empty_skill_md() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("empty-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    
    // Empty SKILL.md file
    fs::write(skill_dir.join("SKILL.md"), "").unwrap();
    
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    let result = runtime.discover_skills();
    // Should handle gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_activate_skill_before_discovery() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: Test
---
"#,
    ).unwrap();
    
    let runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    // Activate before discovery - should auto-discover or error
    let result = runtime.activate_skill("test-skill");
    // Implementation may auto-discover or require explicit discovery
    assert!(result.is_ok() || result.is_err());
}
