use openskills_runtime::{OpenSkillRuntime, ExecutionOptions};
use serde_json::json;
use std::path::PathBuf;
use tempfile::TempDir;

fn get_claude_skills_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join("claude-official-skills")
        .join("skills")
}

#[test]
fn test_load_all_claude_skills() {
    let skills_dir = get_claude_skills_dir();
    if !skills_dir.exists() {
        println!("Skipping test: Claude official skills not found at {:?}", skills_dir);
        return;
    }

    let mut runtime = OpenSkillRuntime::from_directory(&skills_dir);
    let skills = runtime.discover_skills().expect("Failed to discover skills");
    
    println!("Discovered {} skills", skills.len());
    for skill in &skills {
        println!("- {} ({})", skill.id, skill.location);
    }

    assert!(!skills.is_empty(), "Should discover at least one skill");
    
    // Verify some specific known skills exist
    assert!(skills.iter().any(|s| s.id == "skill-creator"));
    assert!(skills.iter().any(|s| s.id == "docx"));
    assert!(skills.iter().any(|s| s.id == "pdf"));
}

#[test]
#[cfg(target_os = "macos")]
fn test_seatbelt_python_init_skill() {
    let skills_dir = get_claude_skills_dir();
    if !skills_dir.exists() {
        println!("Skipping test: Claude official skills not found");
        return;
    }

    // Initialize runtime with the directory containing skill-creator
    let mut runtime = OpenSkillRuntime::from_directory(&skills_dir);
    runtime.discover_skills().unwrap();

    // Verify skill-creator is available
    if !runtime.list_skills().iter().any(|s| s.id == "skill-creator") {
        panic!("skill-creator skill not found");
    }

    // Create a temporary directory for the output
    let temp_dir = TempDir::new().unwrap();
    // unused output_path for now as we just check usage
    let _output_path = temp_dir.path().join("my-new-skill");
    
    let options = ExecutionOptions {
        timeout_ms: Some(10000),
        input: Some(json!({})), // Empty input
        ..Default::default()
    };

    let result = runtime.execute_skill("skill-creator", options);
    
    match result {
        Ok(exec_result) => {
            println!("Output: {}", exec_result.output);
            println!("Stdout: {}", exec_result.stdout);
            println!("Stderr: {}", exec_result.stderr);
            
            // init_skill.py exits with 1 if args are missing.
            // If it executed, we expect "Usage: init_skill.py" in stdout/stderr
            let output_combined = format!("{}{}", exec_result.stdout, exec_result.stderr);
            assert!(output_combined.contains("Usage: init_skill.py"), "Did not find Usage message, script might not have run");
        },
        Err(e) => {
            println!("Execution error: {:?}", e);
            // It might return an error if the exit code is non-zero, depending on runtime implementation details not fully visible here.
            // But based on previous knowledge, `execute_skill` returns Ok with status Failed for non-zero exits.
            // However, if the runtime is strict about exit codes, it might be Err.
            // Let's inspect the error if it happens.
            panic!("Execution failed with error: {:?}", e);
        }
    }
}
