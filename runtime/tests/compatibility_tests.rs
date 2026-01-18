use openskills_runtime::{OpenSkillRuntime, RuntimeConfig};
use serde_json::Value;
use std::path::PathBuf;

fn get_examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join("skills")
}

#[test]
fn test_discover_skills_standard() {
    // This test points to the examples/skills directory as if it were a standard location
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    
    let skills = runtime.discover_skills().expect("Failed to discover skills");
    assert!(!skills.is_empty(), "Should find at least one skill in examples");
    
    let example_skill = skills.iter().find(|s| s.id == "example-skill");
    assert!(example_skill.is_some(), "Should find example-skill");
    
    let skill = example_skill.unwrap();
    assert_eq!(skill.id, "example-skill");
    assert!(!skill.description.is_empty());
}

#[test]
fn test_system_prompt_metadata() {
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    runtime.discover_skills().unwrap();

    let metadata = runtime.get_system_prompt_metadata();
    assert!(metadata.contains("You have access to the following skills:"));
    assert!(metadata.contains("- example-skill:"));
    
    let summary = runtime.get_system_prompt_summary();
    assert!(summary.contains("Skills:"));
    assert!(summary.contains("example-skill"));
}

#[test]
fn test_system_prompt_metadata_json() {
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    runtime.discover_skills().unwrap();

    let json_str = runtime.get_system_prompt_metadata_json().unwrap();
    let json: Value = serde_json::from_str(&json_str).unwrap();
    
    let skills = json["skills"].as_array().unwrap();
    assert!(!skills.is_empty());
    
    let example_skill = skills.iter().find(|s| s["id"] == "example-skill").unwrap();
    assert_eq!(example_skill["user_invocable"], true);
}

#[test]
fn test_runtime_config_builder() {
    let examples_dir = get_examples_dir();
    let config = RuntimeConfig {
        custom_directories: vec![examples_dir.clone()],
        use_standard_locations: false,
        project_root: None,
    };
    
    let mut runtime = OpenSkillRuntime::from_config(config);
    let skills = runtime.discover_skills().unwrap();
    
    assert!(!skills.is_empty());
}
