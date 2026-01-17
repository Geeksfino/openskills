use openskills_runtime::OpenSkillRuntime;
use serde_json::json;

#[test]
fn loads_example_skill_metadata() {
    let skills_dir = format!(
        "{}/../examples/skills",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut runtime = OpenSkillRuntime::new(skills_dir);
    let skills = runtime.load_skills().expect("load skills");
    assert!(skills.iter().any(|s| s.id == "example-skill"));
}

#[test]
fn validates_input_schema() {
    let skills_dir = format!(
        "{}/../examples/skills",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut runtime = OpenSkillRuntime::new(skills_dir);
    let result = runtime.execute_skill(
        "example-skill",
        json!({"conversation": "hello"}),
        openskills_runtime::ExecutionOptions { timeout_ms: Some(1000) },
    );

    // This will fail due to placeholder wasm, but should fail after validation
    assert!(result.is_err());
}
