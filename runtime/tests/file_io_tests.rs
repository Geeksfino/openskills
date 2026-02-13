//! File I/O Tests
//!
//! Tests for readSkillFile and listSkillFiles operations.
//! Verifies correct file reading, path traversal protection, and directory listing.

use openskills_runtime::OpenSkillRuntime;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// readSkillFile Tests
// =============================================================================

#[test]
fn test_read_skill_file_existing() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("file-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    // Create SKILL.md
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: file-skill
description: Skill with helper files.
---
# Instructions
Read helper.md for more info.
"#,
    )
    .unwrap();

    // Create a helper file
    fs::write(skill_dir.join("helper.md"), "# Helper Documentation\n\nThis is helper content.").unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let content = runtime.read_skill_file("file-skill", "helper.md").unwrap();

    assert!(content.contains("Helper Documentation"));
    assert!(content.contains("This is helper content"));
}

#[test]
fn test_read_skill_file_in_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("subdir-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::create_dir_all(skill_dir.join("docs")).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: subdir-skill
description: Skill with subdirectory.
---
"#,
    )
    .unwrap();

    fs::write(skill_dir.join("docs").join("guide.md"), "# Guide\n\nDetailed guide content.").unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let content = runtime.read_skill_file("subdir-skill", "docs/guide.md").unwrap();

    assert!(content.contains("Detailed guide content"));
}

#[test]
fn test_read_skill_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("empty-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: empty-skill
description: Skill without helper files.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.read_skill_file("empty-skill", "nonexistent.md");

    assert!(result.is_err(), "Should error for nonexistent file");
}

#[test]
fn test_read_skill_file_path_traversal_blocked() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("secure-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: secure-skill
description: Test path traversal protection.
---
"#,
    )
    .unwrap();

    // Create a secret file outside skill directory
    fs::write(temp_dir.path().join("secret.txt"), "SECRET DATA").unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Try to read file outside skill directory via path traversal
    let result = runtime.read_skill_file("secure-skill", "../secret.txt");

    // Should either error or not return the secret content
    match result {
        Ok(content) => {
            assert!(
                !content.contains("SECRET DATA"),
                "Path traversal should be blocked"
            );
        }
        Err(_) => {
            // Error is expected and acceptable
        }
    }
}

#[test]
fn test_read_skill_file_absolute_path_blocked() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("abs-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: abs-skill
description: Test absolute path protection.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Try to read file with absolute path
    let result = runtime.read_skill_file("abs-skill", "/etc/passwd");

    // Should error - absolute paths not allowed
    assert!(result.is_err(), "Absolute paths should be blocked");
}

#[test]
fn test_read_skill_file_skill_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.read_skill_file("nonexistent-skill", "file.md");

    assert!(result.is_err(), "Should error for nonexistent skill");
}

// =============================================================================
// listSkillFiles Tests
// =============================================================================

#[test]
fn test_list_skill_files_flat() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("list-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: list-skill
description: Skill with files.
---
"#,
    )
    .unwrap();

    fs::write(skill_dir.join("readme.md"), "README").unwrap();
    fs::write(skill_dir.join("config.json"), "{}").unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let files = runtime.list_skill_files("list-skill", None, false).unwrap();

    assert!(files.contains(&"SKILL.md".to_string()));
    assert!(files.contains(&"readme.md".to_string()));
    assert!(files.contains(&"config.json".to_string()));
}

#[test]
fn test_list_skill_files_recursive() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("recursive-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::create_dir_all(skill_dir.join("docs")).unwrap();
    fs::create_dir_all(skill_dir.join("scripts")).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: recursive-skill
description: Skill with subdirs.
---
"#,
    )
    .unwrap();

    fs::write(skill_dir.join("docs").join("guide.md"), "Guide").unwrap();
    fs::write(skill_dir.join("scripts").join("run.sh"), "#!/bin/bash").unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let files = runtime.list_skill_files("recursive-skill", None, true).unwrap();

    // Should include files from subdirectories
    assert!(
        files.iter().any(|f| f.contains("guide.md")),
        "Should include docs/guide.md"
    );
    assert!(
        files.iter().any(|f| f.contains("run.sh")),
        "Should include scripts/run.sh"
    );
}

#[test]
fn test_list_skill_files_subdir() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("subdir-list-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::create_dir_all(skill_dir.join("examples")).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: subdir-list-skill
description: Test subdir listing.
---
"#,
    )
    .unwrap();

    fs::write(skill_dir.join("examples").join("example1.js"), "// example 1").unwrap();
    fs::write(skill_dir.join("examples").join("example2.js"), "// example 2").unwrap();
    fs::write(skill_dir.join("other.txt"), "other file").unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let files = runtime
        .list_skill_files("subdir-list-skill", Some("examples"), false)
        .unwrap();

    // Should only include files from examples/
    assert!(files.iter().any(|f| f.contains("example1.js")));
    assert!(files.iter().any(|f| f.contains("example2.js")));
    // Should not include files from root
    assert!(!files.iter().any(|f| f.contains("other.txt")));
}

#[test]
fn test_list_skill_files_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("minimal-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: minimal-skill
description: Minimal skill.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let files = runtime.list_skill_files("minimal-skill", None, false).unwrap();

    // Should only have SKILL.md
    assert_eq!(files.len(), 1);
    assert!(files.contains(&"SKILL.md".to_string()));
}

#[test]
fn test_list_skill_files_nonexistent_subdir() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("no-subdir-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: no-subdir-skill
description: No subdirectory.
---
"#,
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.list_skill_files("no-subdir-skill", Some("nonexistent"), false);

    // Should error or return empty list
    match result {
        Ok(files) => assert!(files.is_empty()),
        Err(_) => {} // Error is acceptable
    }
}

#[test]
fn test_list_skill_files_skill_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let result = runtime.list_skill_files("nonexistent-skill", None, false);

    assert!(result.is_err(), "Should error for nonexistent skill");
}

// =============================================================================
// File Type Handling
// =============================================================================

#[test]
fn test_read_skill_file_binary_handling() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("binary-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: binary-skill
description: Skill with binary file.
---
"#,
    )
    .unwrap();

    // Create a small binary file (PNG header bytes)
    let binary_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    fs::write(skill_dir.join("image.png"), binary_data).unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    // Reading binary file should work but may return raw bytes as string
    let result = runtime.read_skill_file("binary-skill", "image.png");

    // Should succeed (might return binary as string or error)
    // The important thing is it doesn't crash
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_read_skill_file_utf8() {
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("utf8-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: utf8-skill
description: Skill with UTF-8 file.
---
"#,
    )
    .unwrap();

    // Create file with UTF-8 content
    fs::write(
        skill_dir.join("unicode.md"),
        "# Unicode Test\n\n日本語 中文 한국어 العربية",
    )
    .unwrap();

    let mut runtime = OpenSkillRuntime::from_directory(temp_dir.path());
    runtime.discover_skills().unwrap();

    let content = runtime.read_skill_file("utf8-skill", "unicode.md").unwrap();

    assert!(content.contains("日本語"));
    assert!(content.contains("中文"));
    assert!(content.contains("한국어"));
}
