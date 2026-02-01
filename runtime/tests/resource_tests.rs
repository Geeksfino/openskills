use openskills_runtime::OpenSkillRuntime;
use std::fs;
use tempfile::TempDir;

fn create_skill_with_files(temp_dir: &TempDir, skill_name: &str) -> std::path::PathBuf {
    let skill_dir = temp_dir.path().join(skill_name);
    fs::create_dir_all(&skill_dir).unwrap();
    
    let skill_md = format!(
        r#"---
name: {}
description: Test skill with files.
allowed-tools: Read, Write
---
# Instructions

This skill has helper files.
"#,
        skill_name
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    // Create helper files
    fs::write(skill_dir.join("helper.md"), "# Helper File\n\nHelper content").unwrap();
    fs::write(skill_dir.join("config.json"), r#"{"key": "value"}"#).unwrap();
    
    // Create subdirectories
    let scripts_dir = skill_dir.join("scripts");
    fs::create_dir_all(&scripts_dir).unwrap();
    fs::write(scripts_dir.join("script.py"), "print('Hello')").unwrap();
    
    let nested_dir = skill_dir.join("nested").join("deep");
    fs::create_dir_all(&nested_dir).unwrap();
    fs::write(nested_dir.join("file.txt"), "deep file").unwrap();
    
    skill_dir
}

#[test]
fn test_read_skill_file_basic() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_files(&temp_dir, "resource-test");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let content = runtime.read_skill_file("resource-test", "helper.md").unwrap();
    assert!(content.contains("Helper File"));
    assert!(content.contains("Helper content"));
}

#[test]
fn test_read_skill_file_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_files(&temp_dir, "resource-test");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let content = runtime.read_skill_file("resource-test", "scripts/script.py").unwrap();
    assert!(content.contains("print('Hello')"));
}

#[test]
fn test_read_skill_file_nested_path() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_files(&temp_dir, "resource-test");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let content = runtime.read_skill_file("resource-test", "nested/deep/file.txt").unwrap();
    assert_eq!(content, "deep file");
}

#[test]
fn test_read_skill_file_path_traversal_protection() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_files(&temp_dir, "resource-test");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Attempt path traversal - should fail
    let result = runtime.read_skill_file("resource-test", "../other-skill/file.txt");
    assert!(result.is_err());
    
    // Another traversal attempt
    let result = runtime.read_skill_file("resource-test", "../../etc/passwd");
    assert!(result.is_err());
    
    // Multiple ../ attempts
    let result = runtime.read_skill_file("resource-test", "../../../etc/passwd");
    assert!(result.is_err());
}

#[test]
fn test_read_skill_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_files(&temp_dir, "resource-test");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.read_skill_file("resource-test", "nonexistent.txt");
    assert!(result.is_err());
}

#[test]
fn test_read_skill_file_skill_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.read_skill_file("nonexistent-skill", "file.txt");
    assert!(result.is_err());
}

#[test]
fn test_list_skill_files_root() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_files(&temp_dir, "resource-test");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let files = runtime.list_skill_files("resource-test", None, false).unwrap();
    // list_skill_files returns files only, not directories (when recursive=false)
    assert!(files.contains(&"helper.md".to_string()));
    assert!(files.contains(&"config.json".to_string()));
    assert!(files.contains(&"SKILL.md".to_string()));
    // Directories are not included in non-recursive listing
}

#[test]
fn test_list_skill_files_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_files(&temp_dir, "resource-test");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let files = runtime.list_skill_files("resource-test", Some("scripts"), false).unwrap();
    // Should find script.py in scripts directory
    assert!(files.iter().any(|f| f.contains("script.py")));
    assert!(!files.is_empty());
}

#[test]
fn test_list_skill_files_recursive() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_files(&temp_dir, "resource-test");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let files = runtime.list_skill_files("resource-test", None, true).unwrap();
    assert!(files.contains(&"helper.md".to_string()));
    assert!(files.contains(&"scripts/script.py".to_string()));
    assert!(files.contains(&"nested/deep/file.txt".to_string()));
}

#[test]
fn test_list_skill_files_nested_recursive() {
    let temp_dir = TempDir::new().unwrap();
    create_skill_with_files(&temp_dir, "resource-test");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let files = runtime.list_skill_files("resource-test", Some("nested"), true).unwrap();
    // Should find nested files recursively
    assert!(files.iter().any(|f| f.contains("file.txt")));
}

#[test]
fn test_get_skill_root() {
    let temp_dir = TempDir::new().unwrap();
    let _skill_dir = create_skill_with_files(&temp_dir, "resource-test");

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let root = runtime.get_skill_root("resource-test").unwrap();
    // Just verify we got a valid path (returns PathBuf)
    assert!(root.exists());
    assert!(root.to_string_lossy().contains("resource-test"));
}

#[test]
fn test_get_skill_root_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.get_skill_root("nonexistent-skill");
    assert!(result.is_err());
}

#[test]
#[cfg(target_os = "macos")]
fn test_run_skill_target_script() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = create_skill_with_files(&temp_dir, "resource-test");
    
    // Create a simple Python script
    let scripts_dir = skill_dir.join("scripts");
    fs::write(scripts_dir.join("test.py"), "import sys; print('Hello from Python')").unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    use openskills_runtime::ExecutionTarget;
    
    let result = runtime.run_skill_target(
        "resource-test",
        ExecutionTarget::Script {
            path: "scripts/test.py".to_string(),
            args: vec![],
        },
        Some(5000),
        None,
        None,
    );

    // Python might not be available, so we accept either success or appropriate error
    match result {
        Ok(exec_result) => {
            assert!(exec_result.stdout.contains("Hello from Python") || 
                    exec_result.stderr.contains("python") ||
                    exec_result.stderr.contains("Python"));
        }
        Err(e) => {
            // Acceptable errors: Python not found, execution failed, etc.
            let error_str = e.to_string();
            assert!(error_str.contains("python") || 
                    error_str.contains("Python") ||
                    error_str.contains("not found") ||
                    error_str.contains("execution"));
        }
    }
}

#[test]
fn test_run_skill_target_skill_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    use openskills_runtime::ExecutionTarget;
    
    let result = runtime.run_skill_target(
        "nonexistent-skill",
        ExecutionTarget::Script {
            path: "script.py".to_string(),
            args: vec![],
        },
        None,
        None,
        None,
    );

    assert!(result.is_err());
}
